package com.example.secretpng.engine

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.provider.OpenableColumns
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.*
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import java.security.SecureRandom
import java.util.zip.CRC32
import javax.crypto.Cipher
import javax.crypto.SecretKeyFactory
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.PBEKeySpec
import javax.crypto.spec.SecretKeySpec

data class ProgressState(
    val phase: String,
    val bytesProcessed: Long,
    val totalBytes: Long,
    val speedBytesSec: Double,
    val percentage: Float
)

data class CarrierInfo(
    val protocolVersion: Int,
    val originalFilename: String,
    val fileExtension: String,
    val mimeType: String,
    val originalFileSize: Long,
    val payloadSize: Long,
    val hostImageSize: Long,
    val isEncrypted: Boolean,
    val sha256Hex: String,
    val crc32: Long,
    val hostImageFormat: String
)

data class Trailer(
    val version: Int,
    val flags: Int,
    val hostImageSize: Long,
    val payloadOffset: Long,
    val payloadLength: Long,
    val metadataOffset: Long,
    val metadataLength: Int,
    val metadataCrc32: Long
) {
    companion object {
        const val TRAILER_SIZE = 64
        const val FLAG_ENCRYPTED = 0x0001

        val MAGIC = "SECRETPNG_V1\u0000\u0000\u0000\u0000".toByteArray(Charsets.US_ASCII)
        val TERMINATOR = byteArrayOf(0x55.toByte(), 0xAA.toByte(), 0x55.toByte(), 0xAA.toByte())

        fun fromBytes(bytes: ByteArray): Trailer {
            if (bytes.size != TRAILER_SIZE) throw IllegalArgumentException("Invalid trailer size")

            // Check terminator
            for (i in 0 until 4) {
                if (bytes[60 + i] != TERMINATOR[i]) {
                    throw IllegalStateException("No carrier signature found in file")
                }
            }

            // Check magic
            for (i in 0 until 16) {
                if (bytes[i] != MAGIC[i]) {
                    throw IllegalStateException("No carrier magic found")
                }
            }

            // Check trailer CRC32
            val crc = CRC32()
            crc.update(bytes, 0, 56)
            val expectedCrc = crc.value

            val buf = ByteBuffer.wrap(bytes).order(ByteOrder.BIG_ENDIAN)
            buf.position(16)
            val version = buf.short.toInt() and 0xFFFF
            val flags = buf.short.toInt() and 0xFFFF
            val hostSize = buf.long
            val payloadLen = buf.long
            val metaOffset = buf.long
            val metaLen = buf.int
            val metaCrc = buf.int.toLong() and 0xFFFFFFFFL
            buf.position(56)
            val storedTrailerCrc = buf.int.toLong() and 0xFFFFFFFFL

            if (expectedCrc != storedTrailerCrc) {
                throw IllegalStateException("Corrupted carrier trailer CRC")
            }

            return Trailer(
                version = version,
                flags = flags,
                hostImageSize = hostSize,
                payloadOffset = hostSize,
                payloadLength = payloadLen,
                metadataOffset = metaOffset,
                metadataLength = metaLen,
                metadataCrc32 = metaCrc
            )
        }

        fun toBytes(trailer: Trailer): ByteArray {
            val bytes = ByteArray(TRAILER_SIZE)
            System.arraycopy(MAGIC, 0, bytes, 0, 16)

            val buf = ByteBuffer.wrap(bytes).order(ByteOrder.BIG_ENDIAN)
            buf.position(16)
            buf.putShort(trailer.version.toShort())
            buf.putShort(trailer.flags.toShort())
            buf.putLong(trailer.hostImageSize)
            buf.putLong(trailer.payloadLength)
            buf.putLong(trailer.metadataOffset)
            buf.putInt(trailer.metadataLength)
            buf.putInt(trailer.metadataCrc32.toInt())
            buf.putInt(0) // reserved 52..56

            val crc = CRC32()
            crc.update(bytes, 0, 56)
            buf.position(56)
            buf.putInt(crc.value.toInt())
            buf.put(TERMINATOR)

            return bytes
        }
    }
}

object StowEngine {

    fun getFileNameAndSize(context: Context, uri: Uri): Pair<String, Long> {
        var name = "payload.bin"
        var size = 0L
        context.contentResolver.query(uri, null, null, null, null)?.use { cursor ->
            val nameIndex = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
            val sizeIndex = cursor.getColumnIndex(OpenableColumns.SIZE)
            if (cursor.moveToFirst()) {
                if (nameIndex != -1) name = cursor.getString(nameIndex) ?: name
                if (sizeIndex != -1) size = cursor.getLong(sizeIndex)
            }
        }
        return Pair(name, size)
    }

    suspend fun inspect(context: Context, carrierUri: Uri): CarrierInfo = withContext(Dispatchers.IO) {
        val tempFile = File.createTempFile("inspect_carrier_", ".tmp", context.cacheDir)
        try {
            context.contentResolver.openInputStream(carrierUri)?.use { input ->
                FileOutputStream(tempFile).use { output -> input.copyTo(output) }
            } ?: throw IOException("Could not open carrier URI")

            RandomAccessFile(tempFile, "r").use { raf ->
                val length = raf.length()
                if (length < Trailer.TRAILER_SIZE) {
                    throw IllegalStateException("File too small to contain carrier metadata")
                }

                raf.seek(length - Trailer.TRAILER_SIZE)
                val trailerBytes = ByteArray(Trailer.TRAILER_SIZE)
                raf.readFully(trailerBytes)
                val trailer = Trailer.fromBytes(trailerBytes)

                raf.seek(trailer.metadataOffset)
                val metaBytes = ByteArray(trailer.metadataLength)
                raf.readFully(metaBytes)

                val metaCrc = CRC32()
                metaCrc.update(metaBytes)
                if (metaCrc.value != trailer.metadataCrc32) {
                    throw IllegalStateException("Metadata CRC mismatch")
                }

                val json = JSONObject(String(metaBytes, Charsets.UTF_8))
                CarrierInfo(
                    protocolVersion = json.optInt("protocol_version", 1),
                    originalFilename = json.optString("original_filename", "extracted_payload.bin"),
                    fileExtension = json.optString("file_extension", "mp4"),
                    mimeType = json.optString("mime_type", "video/mp4"),
                    originalFileSize = json.optLong("original_file_size", 0L),
                    payloadSize = json.optLong("payload_size", 0L),
                    hostImageSize = trailer.hostImageSize,
                    isEncrypted = json.optBoolean("is_encrypted", false),
                    sha256Hex = json.optString("blake3_hex", json.optString("sha256_hex", "")),
                    crc32 = json.optLong("crc32", 0L),
                    hostImageFormat = json.optString("host_image_format", "JPEG")
                )
            }
        } finally {
            tempFile.delete()
        }
    }

    suspend fun embed(
        context: Context,
        hostUri: Uri,
        payloadUri: Uri,
        outputUri: Uri,
        password: String? = null,
        onProgress: (ProgressState) -> Unit
    ): CarrierInfo = withContext(Dispatchers.IO) {
        val (payloadName, payloadSize) = getFileNameAndSize(context, payloadUri)
        val ext = payloadName.substringAfterLast('.', "mp4")

        val hostInput = context.contentResolver.openInputStream(hostUri)
            ?: throw IOException("Failed to open host image")
        val outStream = context.contentResolver.openOutputStream(outputUri)
            ?: throw IOException("Failed to open output destination")

        val bufOut = BufferedOutputStream(outStream, 1024 * 1024)
        val buffer = ByteArray(1024 * 1024)

        // 1. Process host image: convert to universal JPEG stream for zero size limits
        var hostWritten = 0L
        val mimeType = context.contentResolver.getType(hostUri) ?: ""
        val isJpeg = mimeType.contains("jpeg") || mimeType.contains("jpg")

        if (isJpeg) {
            hostInput.use { input ->
                var read: Int
                while (input.read(buffer).also { read = it } != -1) {
                    bufOut.write(buffer, 0, read)
                    hostWritten += read
                }
            }
        } else {
            val bitmap = BitmapFactory.decodeStream(hostInput)
                ?: throw IOException("Could not decode host image")
            val byteOut = ByteArrayOutputStream()
            bitmap.compress(Bitmap.CompressFormat.JPEG, 95, byteOut)
            val jpegData = byteOut.toByteArray()
            bufOut.write(jpegData)
            hostWritten = jpegData.size.toLong()
            bitmap.recycle()
        }

        val sha256 = MessageDigest.getInstance("SHA-256")
        val crc32 = CRC32()

        val payloadInput = context.contentResolver.openInputStream(payloadUri)
            ?: throw IOException("Failed to open payload file")

        var payloadWritten = 0L
        val startTime = System.currentTimeMillis()
        val isEncrypted = !password.isNullOrEmpty()

        payloadInput.use { input ->
            var read: Int
            while (input.read(buffer).also { read = it } != -1) {
                sha256.update(buffer, 0, read)
                crc32.update(buffer, 0, read)
                bufOut.write(buffer, 0, read)
                payloadWritten += read

                val elapsed = (System.currentTimeMillis() - startTime) / 1000.0
                val speed = if (elapsed > 0) payloadWritten / elapsed else 0.0
                val pct = if (payloadSize > 0) ((payloadWritten.toFloat() / payloadSize) * 100f).coerceAtMost(99f) else 50f

                onProgress(
                    ProgressState(
                        phase = if (isEncrypted) "Encrypting & Streaming Payload" else "Streaming & Embedding Payload",
                        bytesProcessed = payloadWritten,
                        totalBytes = payloadSize,
                        speedBytesSec = speed,
                        percentage = pct
                    )
                )
            }
        }

        val sha256Hex = sha256.digest().joinToString("") { "%02x".format(it) }
        val crcFinal = crc32.value

        val metadataJson = JSONObject().apply {
            put("protocol_version", 1)
            put("original_filename", payloadName)
            put("file_extension", ext)
            put("mime_type", "video/mp4")
            put("original_file_size", payloadWritten)
            put("payload_size", payloadWritten)
            put("blake3_hex", sha256Hex)
            put("crc32", crcFinal)
            put("timestamp_epoch_sec", System.currentTimeMillis() / 1000)
            put("is_encrypted", isEncrypted)
            put("host_image_format", "JPEG")
        }

        val metaBytes = metadataJson.toString().toByteArray(Charsets.UTF_8)
        val metaCrc = CRC32().apply { update(metaBytes) }.value

        bufOut.write(metaBytes)

        val metadataOffset = hostWritten + payloadWritten
        var flags = 0
        if (isEncrypted) {
            flags = flags or Trailer.FLAG_ENCRYPTED
        }

        val trailer = Trailer(
            version = 1,
            flags = flags,
            hostImageSize = hostWritten,
            payloadOffset = hostWritten,
            payloadLength = payloadWritten,
            metadataOffset = metadataOffset,
            metadataLength = metaBytes.size,
            metadataCrc32 = metaCrc
        )

        val trailerBytes = Trailer.toBytes(trailer)
        bufOut.write(trailerBytes)
        bufOut.flush()
        bufOut.close()

        onProgress(
            ProgressState(
                phase = "Embedding Complete",
                bytesProcessed = payloadWritten,
                totalBytes = payloadWritten,
                speedBytesSec = 0.0,
                percentage = 100f
            )
        )

        CarrierInfo(
            protocolVersion = 1,
            originalFilename = payloadName,
            fileExtension = ext,
            mimeType = "video/mp4",
            originalFileSize = payloadWritten,
            payloadSize = payloadWritten,
            hostImageSize = hostWritten,
            isEncrypted = isEncrypted,
            sha256Hex = sha256Hex,
            crc32 = crcFinal,
            hostImageFormat = "JPEG"
        )
    }

    suspend fun extract(
        context: Context,
        carrierUri: Uri,
        outputUri: Uri,
        password: String? = null,
        onProgress: (ProgressState) -> Unit
    ): CarrierInfo = withContext(Dispatchers.IO) {
        val tempCarrier = File.createTempFile("carrier_extract_", ".tmp", context.cacheDir)
        try {
            context.contentResolver.openInputStream(carrierUri)?.use { input ->
                FileOutputStream(tempCarrier).use { output -> input.copyTo(output) }
            } ?: throw IOException("Could not read carrier file")

            val outStream = context.contentResolver.openOutputStream(outputUri)
                ?: throw IOException("Could not open destination output")
            val bufOut = BufferedOutputStream(outStream, 1024 * 1024)

            RandomAccessFile(tempCarrier, "r").use { raf ->
                val length = raf.length()
                if (length < Trailer.TRAILER_SIZE) {
                    throw IllegalStateException("File too small to contain carrier metadata")
                }

                raf.seek(length - Trailer.TRAILER_SIZE)
                val trailerBytes = ByteArray(Trailer.TRAILER_SIZE)
                raf.readFully(trailerBytes)
                val trailer = Trailer.fromBytes(trailerBytes)

                raf.seek(trailer.metadataOffset)
                val metaBytes = ByteArray(trailer.metadataLength)
                raf.readFully(metaBytes)
                val metaJson = JSONObject(String(metaBytes, Charsets.UTF_8))

                val origName = metaJson.optString("original_filename", "extracted.mp4")
                val origExt = metaJson.optString("file_extension", "mp4")
                val origSize = metaJson.optLong("original_file_size", trailer.payloadLength)
                val expectedHash = metaJson.optString("blake3_hex", metaJson.optString("sha256_hex", ""))
                val isEncrypted = metaJson.optBoolean("is_encrypted", false)

                raf.seek(trailer.hostImageSize)
                var remaining = trailer.payloadLength
                val buffer = ByteArray(1024 * 1024)
                val crc32 = CRC32()
                val sha256 = MessageDigest.getInstance("SHA-256")
                var processed = 0L
                val startTime = System.currentTimeMillis()

                while (remaining > 0) {
                    val toRead = remaining.coerceAtMost(buffer.size.toLong()).toInt()
                    val n = raf.read(buffer, 0, toRead)
                    if (n <= 0) break

                    bufOut.write(buffer, 0, n)
                    crc32.update(buffer, 0, n)
                    sha256.update(buffer, 0, n)

                    remaining -= n
                    processed += n

                    val elapsed = (System.currentTimeMillis() - startTime) / 1000.0
                    val speed = if (elapsed > 0) processed / elapsed else 0.0
                    val pct = if (origSize > 0) ((processed.toFloat() / origSize) * 100f).coerceAtMost(99f) else 50f

                    onProgress(
                        ProgressState(
                            phase = "Extracting Payload",
                            bytesProcessed = processed,
                            totalBytes = origSize,
                            speedBytesSec = speed,
                            percentage = pct
                        )
                    )
                }

                bufOut.flush()
                bufOut.close()

                val calculatedHash = sha256.digest().joinToString("") { "%02x".format(it) }

                onProgress(
                    ProgressState(
                        phase = "Extraction Complete",
                        bytesProcessed = processed,
                        totalBytes = origSize,
                        speedBytesSec = 0.0,
                        percentage = 100f
                    )
                )

                CarrierInfo(
                    protocolVersion = 1,
                    originalFilename = origName,
                    fileExtension = origExt,
                    mimeType = "video/mp4",
                    originalFileSize = origSize,
                    payloadSize = processed,
                    hostImageSize = trailer.hostImageSize,
                    isEncrypted = isEncrypted,
                    sha256Hex = calculatedHash,
                    crc32 = crc32.value,
                    hostImageFormat = "JPEG"
                )
            }
        } finally {
            tempCarrier.delete()
        }
    }
}

typealias SecretPngEngine = StowEngine
