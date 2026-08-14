// Secret PNG - Flutter / Dart FFI Bridge
// Enables seamless cross-platform embedding and extraction on Android, iOS, Windows, macOS, and Linux.

import 'dart:async';
import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:io';
import 'package:ffi/ffi.dart';

// Native function typedefs
typedef NativeHasCarrier = ffi.Int32 Function(ffi.Pointer<Utf8> carrierPath);
typedef DartHasCarrier = int Function(ffi.Pointer<Utf8> carrierPath);

typedef NativeInspectJson = ffi.Int32 Function(
  ffi.Pointer<Utf8> carrierPath,
  ffi.Pointer<ffi.Pointer<Utf8>> outJson,
);
typedef DartInspectJson = int Function(
  ffi.Pointer<Utf8> carrierPath,
  ffi.Pointer<ffi.Pointer<Utf8>> outJson,
);

typedef NativeProgressCb = ffi.Void Function(
  ffi.Pointer<Utf8> phase,
  ffi.Uint64 bytesProcessed,
  ffi.Uint64 totalBytes,
  ffi.Double speedBytesSec,
  ffi.Float percentage,
  ffi.Pointer<ffi.Void> userData,
);

typedef NativeEmbed = ffi.Int32 Function(
  ffi.Pointer<Utf8> hostPath,
  ffi.Pointer<Utf8> payloadPath,
  ffi.Pointer<Utf8> outputPath,
  ffi.Pointer<Utf8> password,
  ffi.Pointer<ffi.NativeFunction<NativeProgressCb>> progressCb,
  ffi.Pointer<ffi.Void> userData,
  ffi.Pointer<ffi.Pointer<Utf8>> outReportJson,
);
typedef DartEmbed = int Function(
  ffi.Pointer<Utf8> hostPath,
  ffi.Pointer<Utf8> payloadPath,
  ffi.Pointer<Utf8> outputPath,
  ffi.Pointer<Utf8> password,
  ffi.Pointer<ffi.NativeFunction<NativeProgressCb>> progressCb,
  ffi.Pointer<ffi.Void> userData,
  ffi.Pointer<ffi.Pointer<Utf8>> outReportJson,
);

typedef NativeExtract = ffi.Int32 Function(
  ffi.Pointer<Utf8> carrierPath,
  ffi.Pointer<Utf8> outputPath,
  ffi.Pointer<Utf8> password,
  ffi.Pointer<ffi.NativeFunction<NativeProgressCb>> progressCb,
  ffi.Pointer<ffi.Void> userData,
  ffi.Pointer<ffi.Pointer<Utf8>> outReportJson,
);
typedef DartExtract = int Function(
  ffi.Pointer<Utf8> carrierPath,
  ffi.Pointer<Utf8> outputPath,
  ffi.Pointer<Utf8> password,
  ffi.Pointer<ffi.NativeFunction<NativeProgressCb>> progressCb,
  ffi.Pointer<ffi.Void> userData,
  ffi.Pointer<ffi.Pointer<Utf8>> outReportJson,
);

typedef NativeStrip = ffi.Int32 Function(
  ffi.Pointer<Utf8> carrierPath,
  ffi.Pointer<Utf8> outputPath,
  ffi.Pointer<ffi.Pointer<Utf8>> outReportJson,
);
typedef DartStrip = int Function(
  ffi.Pointer<Utf8> carrierPath,
  ffi.Pointer<Utf8> outputPath,
  ffi.Pointer<ffi.Pointer<Utf8>> outReportJson,
);

typedef NativeFreeString = ffi.Void Function(ffi.Pointer<Utf8> ptr);
typedef DartFreeString = void Function(ffi.Pointer<Utf8> ptr);

typedef NativeLastError = ffi.Pointer<Utf8> Function();
typedef DartLastError = ffi.Pointer<Utf8> Function();

/// Progress state update emitted during streaming embedding or extraction
class ProgressInfo {
  final String phase;
  final int bytesProcessed;
  final int totalBytes;
  final double speedBytesSec;
  final double percentage;

  ProgressInfo({
    required this.phase,
    required this.bytesProcessed,
    required this.totalBytes,
    required this.speedBytesSec,
    required this.percentage,
  });
}

/// Carrier metadata inspected in O(1) time
class CarrierMetadata {
  final int protocolVersion;
  final String originalFilename;
  final String fileExtension;
  final String mimeType;
  final int originalFileSize;
  final int payloadSize;
  final int hostImageSize;
  final bool isEncrypted;
  final String blake3Hex;
  final int crc32;
  final String hostImageFormat;
  final int? hostImageWidth;
  final int? hostImageHeight;

  CarrierMetadata.fromJson(Map<String, dynamic> json)
      : protocolVersion = json['protocol_version'] ?? 1,
        originalFilename = json['original_filename'] ?? '',
        fileExtension = json['file_extension'] ?? '',
        mimeType = json['mime_type'] ?? 'application/octet-stream',
        originalFileSize = json['original_file_size'] ?? 0,
        payloadSize = json['payload_size'] ?? 0,
        hostImageSize = json['host_image_size'] ?? 0,
        isEncrypted = json['is_encrypted'] ?? false,
        blake3Hex = json['blake3_hex'] ?? '',
        crc32 = json['crc32'] ?? 0,
        hostImageFormat = json['host_image_format'] ?? '',
        hostImageWidth = json['host_image_width'],
        hostImageHeight = json['host_image_height'];
}

/// Main Secret PNG Engine Client
class SecretPngEngine {
  late final ffi.DynamicLibrary _dylib;
  late final DartHasCarrier _hasCarrier;
  late final DartInspectJson _inspectJson;
  late final DartEmbed _embed;
  late final DartExtract _extract;
  late final DartStrip _strip;
  late final DartFreeString _freeString;
  late final DartLastError _lastError;

  SecretPngEngine({String? customLibraryPath}) {
    if (customLibraryPath != null) {
      _dylib = ffi.DynamicLibrary.open(customLibraryPath);
    } else if (Platform.isAndroid) {
      _dylib = ffi.DynamicLibrary.open("libsecret_png_ffi.so");
    } else if (Platform.isIOS || Platform.isMacOS) {
      _dylib = ffi.DynamicLibrary.process();
    } else if (Platform.isWindows) {
      _dylib = ffi.DynamicLibrary.open("secret_png_ffi.dll");
    } else if (Platform.isLinux) {
      _dylib = ffi.DynamicLibrary.open("libsecret_png_ffi.so");
    } else {
      throw UnsupportedError("Unsupported platform for Secret PNG FFI");
    }

    _hasCarrier = _dylib.lookupFunction<NativeHasCarrier, DartHasCarrier>('secret_png_has_carrier');
    _inspectJson = _dylib.lookupFunction<NativeInspectJson, DartInspectJson>('secret_png_inspect_json');
    _embed = _dylib.lookupFunction<NativeEmbed, DartEmbed>('secret_png_embed');
    _extract = _dylib.lookupFunction<NativeExtract, DartExtract>('secret_png_extract');
    _strip = _dylib.lookupFunction<NativeStrip, DartStrip>('secret_png_strip');
    _freeString = _dylib.lookupFunction<NativeFreeString, DartFreeString>('secret_png_free_string');
    _lastError = _dylib.lookupFunction<NativeLastError, DartLastError>('secret_png_last_error');
  }

  String _getLastError() {
    final ptr = _lastError();
    if (ptr.address == 0) return "Unknown error";
    final msg = ptr.toDartString();
    _freeString(ptr);
    return msg;
  }

  /// Check if an image contains embedded carrier payload
  bool hasCarrier(String imagePath) {
    final cPath = imagePath.toNativeUtf8();
    try {
      final res = _hasCarrier(cPath);
      return res == 1;
    } finally {
      calloc.free(cPath);
    }
  }

  /// Inspect carrier metadata in O(1) time
  CarrierMetadata inspect(String imagePath) {
    final cPath = imagePath.toNativeUtf8();
    final outPtr = calloc<ffi.Pointer<Utf8>>();
    try {
      final res = _inspectJson(cPath, outPtr);
      if (res != 0) {
        throw Exception("Inspection failed: ${_getLastError()}");
      }
      final jsonStr = outPtr.value.toDartString();
      _freeString(outPtr.value);
      final map = jsonDecode(jsonStr) as Map<String, dynamic>;
      return CarrierMetadata.fromJson(map);
    } finally {
      calloc.free(cPath);
      calloc.free(outPtr);
    }
  }

  /// Embed video into host image
  Future<Map<String, dynamic>> embed({
    required String hostPath,
    required String payloadPath,
    required String outputPath,
    String? password,
    void Function(ProgressInfo info)? onProgress,
  }) async {
    final cHost = hostPath.toNativeUtf8();
    final cPayload = payloadPath.toNativeUtf8();
    final cOut = outputPath.toNativeUtf8();
    final cPass = password != null ? password.toNativeUtf8() : ffi.Pointer<Utf8>.fromAddress(0);
    final outReport = calloc<ffi.Pointer<Utf8>>();

    try {
      final res = _embed(
        cHost,
        cPayload,
        cOut,
        cPass,
        ffi.Pointer.fromAddress(0), // Can be wired to NativeCallable for async progress
        ffi.Pointer.fromAddress(0),
        outReport,
      );

      if (res != 0) {
        throw Exception("Embedding failed: ${_getLastError()}");
      }

      final reportStr = outReport.value.toDartString();
      _freeString(outReport.value);
      return jsonDecode(reportStr) as Map<String, dynamic>;
    } finally {
      calloc.free(cHost);
      calloc.free(cPayload);
      calloc.free(cOut);
      if (cPass.address != 0) calloc.free(cPass);
      calloc.free(outReport);
    }
  }

  /// Extract video from carrier image
  Future<Map<String, dynamic>> extract({
    required String carrierPath,
    String? outputPath,
    String? password,
  }) async {
    final cCarrier = carrierPath.toNativeUtf8();
    final cOut = outputPath != null ? outputPath.toNativeUtf8() : ffi.Pointer<Utf8>.fromAddress(0);
    final cPass = password != null ? password.toNativeUtf8() : ffi.Pointer<Utf8>.fromAddress(0);
    final outReport = calloc<ffi.Pointer<Utf8>>();

    try {
      final res = _extract(
        cCarrier,
        cOut,
        cPass,
        ffi.Pointer.fromAddress(0),
        ffi.Pointer.fromAddress(0),
        outReport,
      );

      if (res != 0) {
        throw Exception("Extraction failed: ${_getLastError()}");
      }

      final reportStr = outReport.value.toDartString();
      _freeString(outReport.value);
      return jsonDecode(reportStr) as Map<String, dynamic>;
    } finally {
      calloc.free(cCarrier);
      if (cOut.address != 0) calloc.free(cOut);
      if (cPass.address != 0) calloc.free(cPass);
      calloc.free(outReport);
    }
  }
}
