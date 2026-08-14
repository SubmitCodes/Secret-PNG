package com.secretpng.engine

import android.content.Context
import android.net.Uri
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream

/**
 * Secret PNG Android Scoped Storage & JNI Bridge
 * Handles Content URIs (Storage Access Framework / Scoped Storage) and delegates to native Rust engine.
 */
class SecretPngEngine private constructor() {

    companion object {
        init {
            System.loadLibrary("secret_png_ffi")
        }

        val instance: SecretPngEngine by lazy { SecretPngEngine() }
    }

    // Native C-ABI declarations
    private external fun secretPngHasCarrier(carrierPath: String): Int
    private external fun secretPngInspectJson(carrierPath: String): String?
    private external fun secretPngEmbed(
        hostPath: String,
        payloadPath: String,
        outputPath: String,
        password: String?
    ): String?
    private external fun secretPngExtract(
        carrierPath: String,
        outputPath: String?,
        password: String?
    ): String?
    private external fun secretPngStrip(
        carrierPath: String,
        outputPath: String
    ): String?
    private external fun secretPngLastError(): String?

    /**
     * Resolves an Android Content URI (e.g. content://media/...) into a cache file for native streaming
     */
    private suspend fun copyUriToTemp(context: Context, uri: Uri, prefix: String, extension: String): File =
        withContext(Dispatchers.IO) {
            val tempFile = File.createTempFile(prefix, ".$extension", context.cacheDir)
            context.contentResolver.openInputStream(uri)?.use { input ->
                FileOutputStream(tempFile).use { output ->
                    input.copyTo(output)
                }
            } ?: throw IllegalStateException("Failed to open input stream for URI: $uri")
            tempFile
        }

    /**
     * Embed a video selected via Android SAF into an image file
     */
    suspend fun embedVideo(
        context: Context,
        hostImageUri: Uri,
        videoUri: Uri,
        outputFile: File,
        password: String? = null
    ): JSONObject = withContext(Dispatchers.IO) {
        val tempHost = copyUriToTemp(context, hostImageUri, "host_img_", "png")
        val tempVideo = copyUriToTemp(context, videoUri, "secret_vid_", "mp4")

        try {
            val reportJson = secretPngEmbed(
                tempHost.absolutePath,
                tempVideo.absolutePath,
                outputFile.absolutePath,
                password
            ) ?: throw RuntimeException("Embedding failed: ${secretPngLastError() ?: "Unknown error"}")

            JSONObject(reportJson)
        } finally {
            tempHost.delete()
            tempVideo.delete()
        }
    }

    /**
     * Extract video from a carrier image
     */
    suspend fun extractVideo(
        context: Context,
        carrierImageUri: Uri,
        outputVideoFile: File,
        password: String? = null
    ): JSONObject = withContext(Dispatchers.IO) {
        val tempCarrier = copyUriToTemp(context, carrierImageUri, "carrier_img_", "png")

        try {
            val reportJson = secretPngExtract(
                tempCarrier.absolutePath,
                outputVideoFile.absolutePath,
                password
            ) ?: throw RuntimeException("Extraction failed: ${secretPngLastError() ?: "Unknown error"}")

            JSONObject(reportJson)
        } finally {
            tempCarrier.delete()
        }
    }

    /**
     * Inspect carrier image in O(1) time
     */
    suspend fun inspectCarrier(
        context: Context,
        carrierUri: Uri
    ): JSONObject = withContext(Dispatchers.IO) {
        val tempCarrier = copyUriToTemp(context, carrierUri, "carrier_inspect_", "png")
        try {
            val jsonStr = secretPngInspectJson(tempCarrier.absolutePath)
                ?: throw RuntimeException("Inspection failed: ${secretPngLastError() ?: "No carrier found"}")
            JSONObject(jsonStr)
        } finally {
            tempCarrier.delete()
        }
    }
}
