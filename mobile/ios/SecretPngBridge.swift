// Secret PNG - iOS Swift Bridge
// Handles security-scoped resource URLs from UIDocumentPickerViewController and bridges to Rust C-ABI

import Foundation

public struct CarrierMetadata {
    public let protocolVersion: Int
    public let originalFilename: String
    public let fileExtension: String
    public let mimeType: String
    public let originalFileSize: UInt64
    public let payloadSize: UInt64
    public let hostImageSize: UInt64
    public let isEncrypted: Bool
    public let blake3Hex: String
    public let crc32: UInt32
    public let hostImageFormat: String
}

public class SecretPngBridge {
    public static let shared = SecretPngBridge()

    private init() {}

    /// Inspect carrier image URL in O(1) time
    public func inspectCarrier(url: URL) throws -> CarrierMetadata {
        guard url.startAccessingSecurityScopedResource() else {
            throw NSError(domain: "SecretPng", code: 1, userInfo: [NSLocalizedDescriptionKey: "Failed to access security-scoped URL"])
        }
        defer { url.stopAccessingSecurityScopedResource() }

        let path = url.path
        var outJsonPtr: UnsafeMutablePointer<CChar>? = nil

        let res = secret_png_inspect_json(path, &outJsonPtr)
        guard res == 0, let jsonPtr = outJsonPtr else {
            let errorMsg = self.getLastError()
            throw NSError(domain: "SecretPng", code: Int(res), userInfo: [NSLocalizedDescriptionKey: errorMsg])
        }
        defer { secret_png_free_string(jsonPtr) }

        let jsonString = String(cString: jsonPtr)
        guard let data = jsonString.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw NSError(domain: "SecretPng", code: 2, userInfo: [NSLocalizedDescriptionKey: "Malformed metadata JSON"])
        }

        return CarrierMetadata(
            protocolVersion: json["protocol_version"] as? Int ?? 1,
            originalFilename: json["original_filename"] as? String ?? "",
            fileExtension: json["file_extension"] as? String ?? "",
            mimeType: json["mime_type"] as? String ?? "video/mp4",
            originalFileSize: json["original_file_size"] as? UInt64 ?? 0,
            payloadSize: json["payload_size"] as? UInt64 ?? 0,
            hostImageSize: json["host_image_size"] as? UInt64 ?? 0,
            isEncrypted: json["is_encrypted"] as? Bool ?? false,
            blake3Hex: json["blake3_hex"] as? String ?? "",
            crc32: json["crc32"] as? UInt32 ?? 0,
            hostImageFormat: json["host_image_format"] as? String ?? "PNG"
        )
    }

    /// Embed video into host image
    public func embedVideo(
        hostUrl: URL,
        videoUrl: URL,
        outputUrl: URL,
        password: String? = nil,
        progressHandler: ((_ phase: String, _ percentage: Float) -> Void)? = nil
    ) throws -> [String: Any] {
        guard hostUrl.startAccessingSecurityScopedResource() else {
            throw NSError(domain: "SecretPng", code: 10, userInfo: [NSLocalizedDescriptionKey: "Cannot access host URL"])
        }
        defer { hostUrl.stopAccessingSecurityScopedResource() }

        guard videoUrl.startAccessingSecurityScopedResource() else {
            throw NSError(domain: "SecretPng", code: 11, userInfo: [NSLocalizedDescriptionKey: "Cannot access video URL"])
        }
        defer { videoUrl.stopAccessingSecurityScopedResource() }

        var outReportPtr: UnsafeMutablePointer<CChar>? = nil
        let cPass = password?.cString(using: .utf8)

        let res = secret_png_embed(
            hostUrl.path,
            videoUrl.path,
            outputUrl.path,
            cPass,
            nil,
            nil,
            &outReportPtr
        )

        guard res == 0, let reportPtr = outReportPtr else {
            let errorMsg = self.getLastError()
            throw NSError(domain: "SecretPng", code: Int(res), userInfo: [NSLocalizedDescriptionKey: errorMsg])
        }
        defer { secret_png_free_string(reportPtr) }

        let reportStr = String(cString: reportPtr)
        guard let data = reportStr.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return [:]
        }
        return json
    }

    /// Extract video payload
    public func extractVideo(
        carrierUrl: URL,
        outputUrl: URL,
        password: String? = nil
    ) throws -> [String: Any] {
        guard carrierUrl.startAccessingSecurityScopedResource() else {
            throw NSError(domain: "SecretPng", code: 20, userInfo: [NSLocalizedDescriptionKey: "Cannot access carrier URL"])
        }
        defer { carrierUrl.stopAccessingSecurityScopedResource() }

        var outReportPtr: UnsafeMutablePointer<CChar>? = nil
        let cPass = password?.cString(using: .utf8)

        let res = secret_png_extract(
            carrierUrl.path,
            outputUrl.path,
            cPass,
            nil,
            nil,
            &outReportPtr
        )

        guard res == 0, let reportPtr = outReportPtr else {
            let errorMsg = self.getLastError()
            throw NSError(domain: "SecretPng", code: Int(res), userInfo: [NSLocalizedDescriptionKey: errorMsg])
        }
        defer { secret_png_free_string(reportPtr) }

        let reportStr = String(cString: reportPtr)
        guard let data = reportStr.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return [:]
        }
        return json
    }

    private func getLastError() -> String {
        guard let errPtr = secret_png_last_error() else { return "Unknown native error" }
        defer { secret_png_free_string(errPtr) }
        return String(cString: errPtr)
    }
}
