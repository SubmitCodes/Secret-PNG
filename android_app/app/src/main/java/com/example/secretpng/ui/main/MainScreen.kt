package com.example.secretpng.ui.main

import android.net.Uri
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.example.secretpng.engine.CarrierInfo
import com.example.secretpng.engine.ProgressState
import com.example.secretpng.engine.SecretPngEngine
import kotlinx.coroutines.launch

@Composable
fun MainScreen(modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    var selectedTab by remember { mutableStateOf(0) }
    val tabs = listOf("📦 Embed", "🔓 Extract", "🔍 Inspect")

    // State for Embed
    var hostImageUri by remember { mutableStateOf<Uri?>(null) }
    var hostImageName by remember { mutableStateOf<String?>(null) }
    var payloadVideoUri by remember { mutableStateOf<Uri?>(null) }
    var payloadVideoName by remember { mutableStateOf<String?>(null) }

    // State for Extract / Inspect
    var carrierUri by remember { mutableStateOf<Uri?>(null) }
    var carrierName by remember { mutableStateOf<String?>(null) }
    var inspectedInfo by remember { mutableStateOf<CarrierInfo?>(null) }

    var isProcessing by remember { mutableStateOf(false) }
    var progressState by remember { mutableStateOf<ProgressState?>(null) }
    var lastReport by remember { mutableStateOf<CarrierInfo?>(null) }

    // Pickers
    val pickHostImageLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.GetContent()
    ) { uri: Uri? ->
        if (uri != null) {
            hostImageUri = uri
            hostImageName = SecretPngEngine.getFileNameAndSize(context, uri).first
        }
    }

    val pickPayloadVideoLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.GetContent()
    ) { uri: Uri? ->
        if (uri != null) {
            payloadVideoUri = uri
            payloadVideoName = SecretPngEngine.getFileNameAndSize(context, uri).first
        }
    }

    val pickCarrierLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.GetContent()
    ) { uri: Uri? ->
        if (uri != null) {
            carrierUri = uri
            carrierName = SecretPngEngine.getFileNameAndSize(context, uri).first
            scope.launch {
                try {
                    val info = SecretPngEngine.inspect(context, uri)
                    inspectedInfo = info
                } catch (e: Exception) {
                    inspectedInfo = null
                    Toast.makeText(context, "No carrier payload: ${e.message}", Toast.LENGTH_SHORT).show()
                }
            }
        }
    }

    val saveCarrierLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.CreateDocument("image/png")
    ) { outUri: Uri? ->
        if (outUri != null && hostImageUri != null && payloadVideoUri != null) {
            isProcessing = true
            scope.launch {
                try {
                    val info = SecretPngEngine.embed(
                        context = context,
                        hostUri = hostImageUri!!,
                        payloadUri = payloadVideoUri!!,
                        outputUri = outUri,
                        onProgress = { progressState = it }
                    )
                    lastReport = info
                    Toast.makeText(context, "Carrier image created successfully!", Toast.LENGTH_LONG).show()
                } catch (e: Exception) {
                    Toast.makeText(context, "Error: ${e.message}", Toast.LENGTH_LONG).show()
                } finally {
                    isProcessing = false
                    progressState = null
                }
            }
        }
    }

    val saveExtractedLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.CreateDocument("video/mp4")
    ) { outUri: Uri? ->
        if (outUri != null && carrierUri != null) {
            isProcessing = true
            scope.launch {
                try {
                    val info = SecretPngEngine.extract(
                        context = context,
                        carrierUri = carrierUri!!,
                        outputUri = outUri,
                        onProgress = { progressState = it }
                    )
                    lastReport = info
                    Toast.makeText(context, "Video extracted successfully!", Toast.LENGTH_LONG).show()
                } catch (e: Exception) {
                    Toast.makeText(context, "Extraction error: ${e.message}", Toast.LENGTH_LONG).show()
                } finally {
                    isProcessing = false
                    progressState = null
                }
            }
        }
    }

    fun formatBytes(bytes: Long): String {
        return when {
            bytes >= 1024 * 1024 * 1024 -> "%.2f GB".format(bytes / (1024.0 * 1024 * 1024))
            bytes >= 1024 * 1024 -> "%.2f MB".format(bytes / (1024.0 * 1024))
            bytes >= 1024 -> "%.2f KB".format(bytes / 1024.0)
            else -> "$bytes B"
        }
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(Color(0xFF0D1117))
            .padding(16.dp)
            .verticalScroll(rememberScrollState())
    ) {
        // Header
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.padding(bottom = 12.dp)
        ) {
            Text(
                text = "🛡️ SECRET PNG",
                fontSize = 24.sp,
                fontWeight = FontWeight.Bold,
                color = Color(0xFF38BDF8)
            )
            Spacer(modifier = Modifier.weight(1f))
            Text(
                text = "v1.0 Android",
                fontSize = 12.sp,
                color = Color(0xFF94A3B8)
            )
        }

        // Tab Navigation
        TabRow(
            selectedTabIndex = selectedTab,
            containerColor = Color(0xFF161B22),
            contentColor = Color(0xFF38BDF8),
            modifier = Modifier.clip(RoundedCornerShape(8.dp))
        ) {
            tabs.forEachIndexed { index, title ->
                Tab(
                    selected = selectedTab == index,
                    onClick = { selectedTab = index },
                    text = { Text(title, fontWeight = FontWeight.SemiBold) }
                )
            }
        }

        Spacer(modifier = Modifier.height(16.dp))

        // Progress Card
        progressState?.let { progress ->
            Card(
                colors = CardDefaults.cardColors(containerColor = Color(0xFF0F172A)),
                shape = RoundedCornerShape(8.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .border(1.dp, Color(0xFF38BDF8), RoundedCornerShape(8.dp))
                    .padding(bottom = 16.dp)
            ) {
                Column(modifier = Modifier.padding(12.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text(progress.phase, color = Color(0xFF38BDF8), fontWeight = FontWeight.Bold)
                        Text("%.1f%%".format(progress.percentage), color = Color.White)
                    }
                    Spacer(modifier = Modifier.height(8.dp))
                    LinearProgressIndicator(
                        progress = { progress.percentage / 100f },
                        modifier = Modifier.fillMaxWidth(),
                        color = Color(0xFF38BDF8),
                        trackColor = Color(0xFF334155),
                    )
                    Spacer(modifier = Modifier.height(6.dp))
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text(
                            "${formatBytes(progress.bytesProcessed)} / ${formatBytes(progress.totalBytes)}",
                            fontSize = 12.sp,
                            color = Color(0xFF94A3B8)
                        )
                        Text(
                            "⚡ ${formatBytes(progress.speedBytesSec.toLong())}/s",
                            fontSize = 12.sp,
                            color = Color(0xFF34D399)
                        )
                    }
                }
            }
        }

        when (selectedTab) {
            0 -> { // EMBED
                Card(
                    colors = CardDefaults.cardColors(containerColor = Color(0xFF161B22)),
                    shape = RoundedCornerShape(8.dp),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Column(modifier = Modifier.padding(14.dp)) {
                        Text("🖼️ Select Host Cover Image", fontWeight = FontWeight.Bold, color = Color.White)
                        Spacer(modifier = Modifier.height(6.dp))
                        Button(
                            onClick = { pickHostImageLauncher.launch("image/*") },
                            enabled = !isProcessing,
                            colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF334155))
                        ) {
                            Text(hostImageName ?: "Browse Cover Image...")
                        }

                        Spacer(modifier = Modifier.height(14.dp))

                        Text("🎬 Select Video Payload", fontWeight = FontWeight.Bold, color = Color.White)
                        Spacer(modifier = Modifier.height(6.dp))
                        Button(
                            onClick = { pickPayloadVideoLauncher.launch("video/*") },
                            enabled = !isProcessing,
                            colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF334155))
                        ) {
                            Text(payloadVideoName ?: "Browse Video File...")
                        }

                        Spacer(modifier = Modifier.height(20.dp))

                        Button(
                            onClick = {
                                val stem = hostImageName?.substringBeforeLast('.') ?: "carrier"
                                saveCarrierLauncher.launch("${stem}_carrier.png")
                            },
                            enabled = !isProcessing && hostImageUri != null && payloadVideoUri != null,
                            colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF38BDF8)),
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(48.dp)
                        ) {
                            Text("🚀 Embed Video into Image", color = Color.Black, fontWeight = FontWeight.Bold)
                        }
                    }
                }
            }
            1 -> { // EXTRACT
                Card(
                    colors = CardDefaults.cardColors(containerColor = Color(0xFF161B22)),
                    shape = RoundedCornerShape(8.dp),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Column(modifier = Modifier.padding(14.dp)) {
                        Text("🖼️ Select Carrier Image", fontWeight = FontWeight.Bold, color = Color.White)
                        Spacer(modifier = Modifier.height(6.dp))
                        Button(
                            onClick = { pickCarrierLauncher.launch("image/*") },
                            enabled = !isProcessing,
                            colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF334155))
                        ) {
                            Text(carrierName ?: "Browse Carrier Image...")
                        }

                        inspectedInfo?.let { info ->
                            Spacer(modifier = Modifier.height(12.dp))
                            Card(
                                colors = CardDefaults.cardColors(containerColor = Color(0xFF0F172A)),
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .border(1.dp, Color(0xFF38BDF8), RoundedCornerShape(6.dp))
                                    .padding(8.dp)
                            ) {
                                Column(modifier = Modifier.padding(8.dp)) {
                                    Text("📦 Embedded: ${info.originalFilename}", color = Color(0xFF38BDF8), fontWeight = FontWeight.Bold)
                                    Text("• Size: ${formatBytes(info.originalFileSize)}", color = Color.White, fontSize = 13.sp)
                                    Text("• Cover Size: ${formatBytes(info.hostImageSize)}", color = Color.White, fontSize = 13.sp)
                                    Text("• Integrity: SHA-256 Verified", color = Color(0xFF34D399), fontSize = 13.sp)
                                }
                            }

                            Spacer(modifier = Modifier.height(16.dp))

                            Button(
                                onClick = { saveExtractedLauncher.launch(info.originalFilename) },
                                enabled = !isProcessing,
                                colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF34D399)),
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .height(48.dp)
                            ) {
                                Text("🔓 Extract & Save Video", color = Color.Black, fontWeight = FontWeight.Bold)
                            }
                        }
                    }
                }
            }
            2 -> { // INSPECT
                Card(
                    colors = CardDefaults.cardColors(containerColor = Color(0xFF161B22)),
                    shape = RoundedCornerShape(8.dp),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Column(modifier = Modifier.padding(14.dp)) {
                        Text("🔍 Carrier Inspector", fontWeight = FontWeight.Bold, color = Color.White)
                        Spacer(modifier = Modifier.height(6.dp))
                        Button(
                            onClick = { pickCarrierLauncher.launch("image/*") },
                            enabled = !isProcessing,
                            colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF334155))
                        ) {
                            Text(carrierName ?: "Select Image to Inspect...")
                        }

                        inspectedInfo?.let { info ->
                            Spacer(modifier = Modifier.height(12.dp))
                            Text("Protocol Version: v${info.protocolVersion}", color = Color(0xFF94A3B8))
                            Text("Payload Name: ${info.originalFilename}", color = Color.White)
                            Text("Payload Size: ${formatBytes(info.originalFileSize)}", color = Color.White)
                            Text("Cover Size: ${formatBytes(info.hostImageSize)}", color = Color.White)
                            Text("Fixed Trailer: 64 bytes (EOF)", color = Color(0xFF94A3B8))
                            Text("Checksum: ${info.sha256Hex}", color = Color(0xFF38BDF8), fontSize = 11.sp)
                        }
                    }
                }
            }
        }

        lastReport?.let { report ->
            Spacer(modifier = Modifier.height(16.dp))
            Card(
                colors = CardDefaults.cardColors(containerColor = Color(0xFF064E3B)),
                shape = RoundedCornerShape(8.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .border(1.dp, Color(0xFF10B981), RoundedCornerShape(8.dp))
            ) {
                Column(modifier = Modifier.padding(12.dp)) {
                    Text("✅ Operation Successful!", fontWeight = FontWeight.Bold, color = Color(0xFFA7F3D0))
                    Text("• File: ${report.originalFilename}", color = Color.White, fontSize = 13.sp)
                    Text("• Size: ${formatBytes(report.originalFileSize)}", color = Color.White, fontSize = 13.sp)
                    Text("• Verified SHA-256 Checksum", color = Color(0xFFA7F3D0), fontSize = 12.sp)
                }
            }
        }
    }
}
