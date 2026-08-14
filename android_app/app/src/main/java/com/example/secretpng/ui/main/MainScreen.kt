package com.example.secretpng.ui.main

import android.net.Uri
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import com.example.secretpng.R
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.example.secretpng.engine.CarrierInfo
import com.example.secretpng.engine.ProgressState
import com.example.secretpng.engine.SecretPngEngine
import kotlinx.coroutines.launch

// Color Palette matching Desktop App
val BgColor = Color(0xFF0B0F19)
val CardBg = Color(0xFF161B22)
val CardBorder = Color(0xFF30363D)
val AccentCyan = Color(0xFF38BDF8)
val AccentBlue = Color(0xFF0284C7)
val EmeraldSuccess = Color(0xFF10B981)
val TextMuted = Color(0xFF94A3B8)
val DarkNavy = Color(0xFF0F172A)

@OptIn(ExperimentalMaterial3Api::class)
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
    var embedPassword by remember { mutableStateOf("") }
    var enableEncryption by remember { mutableStateOf(false) }
    var showEmbedPassword by remember { mutableStateOf(false) }

    // State for Extract / Inspect
    var carrierUri by remember { mutableStateOf<Uri?>(null) }
    var carrierName by remember { mutableStateOf<String?>(null) }
    var extractPassword by remember { mutableStateOf("") }
    var showExtractPassword by remember { mutableStateOf(false) }
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
                    Toast.makeText(context, "No carrier payload found: ${e.message}", Toast.LENGTH_SHORT).show()
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
                        password = if (enableEncryption) embedPassword else null,
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
                        password = if (extractPassword.isNotEmpty()) extractPassword else null,
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

    fun formatSpeed(speed: Double): String {
        return when {
            speed >= 1024 * 1024 * 1024 -> "%.2f GB/s".format(speed / (1024.0 * 1024 * 1024))
            speed >= 1024 * 1024 -> "%.2f MB/s".format(speed / (1024.0 * 1024))
            speed >= 1024 -> "%.2f KB/s".format(speed / 1024.0)
            else -> "%.0f B/s".format(speed)
        }
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(BgColor)
            .padding(16.dp)
            .verticalScroll(rememberScrollState())
    ) {
        // Header
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.padding(bottom = 14.dp)
        ) {
            Image(
                painter = painterResource(id = R.drawable.stow_logo),
                contentDescription = "Stow Logo",
                modifier = Modifier
                    .size(28.dp)
                    .clip(RoundedCornerShape(6.dp))
            )
            Spacer(modifier = Modifier.width(8.dp))
            Text(
                text = "STOW",
                fontSize = 22.sp,
                fontWeight = FontWeight.Bold,
                color = AccentCyan
            )
            Spacer(modifier = Modifier.weight(1f))
            Text(
                text = "v1.0",
                fontSize = 12.sp,
                color = TextMuted
            )
        }

        // Tab Navigation
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .background(CardBg, RoundedCornerShape(8.dp))
                .border(1.dp, CardBorder, RoundedCornerShape(8.dp))
                .padding(4.dp),
            horizontalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            tabs.forEachIndexed { index, title ->
                val isSelected = selectedTab == index
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .clip(RoundedCornerShape(6.dp))
                        .background(if (isSelected) DarkNavy else Color.Transparent)
                        .border(
                            1.dp,
                            if (isSelected) AccentCyan else Color.Transparent,
                            RoundedCornerShape(6.dp)
                        )
                        .clickable { selectedTab = index }
                        .padding(vertical = 10.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = title,
                        fontSize = 13.sp,
                        fontWeight = if (isSelected) FontWeight.Bold else FontWeight.Medium,
                        color = if (isSelected) AccentCyan else TextMuted
                    )
                }
            }
        }

        Spacer(modifier = Modifier.height(14.dp))

        // Progress Card
        progressState?.let { progress ->
            Card(
                colors = CardDefaults.cardColors(containerColor = DarkNavy),
                shape = RoundedCornerShape(8.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .border(1.dp, AccentCyan, RoundedCornerShape(8.dp))
                    .padding(bottom = 14.dp)
            ) {
                Column(modifier = Modifier.padding(14.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            text = progress.phase,
                            color = AccentCyan,
                            fontWeight = FontWeight.Bold,
                            fontSize = 14.sp
                        )
                        Spacer(modifier = Modifier.weight(1f))
                        Text(
                            text = "%.1f%%".format(progress.percentage),
                            color = Color.White,
                            fontWeight = FontWeight.Bold,
                            fontSize = 14.sp
                        )
                    }

                    Spacer(modifier = Modifier.height(8.dp))
                    LinearProgressIndicator(
                        progress = { progress.percentage / 100f },
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(8.dp)
                            .clip(RoundedCornerShape(4.dp)),
                        color = AccentCyan,
                        trackColor = Color(0xFF1E293B)
                    )

                    Spacer(modifier = Modifier.height(8.dp))
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            text = "${formatBytes(progress.bytesProcessed)} / ${formatBytes(progress.totalBytes)}",
                            color = TextMuted,
                            fontSize = 12.sp
                        )
                        Spacer(modifier = Modifier.weight(1f))
                        Text(
                            text = "⚡ ${formatSpeed(progress.speedBytesSec)}",
                            color = EmeraldSuccess,
                            fontWeight = FontWeight.Bold,
                            fontSize = 12.sp
                        )
                    }
                }
            }
        }

        // TAB 1: EMBED
        if (selectedTab == 0) {
            // Host Cover Image Card
            Card(
                colors = CardDefaults.cardColors(containerColor = CardBg),
                shape = RoundedCornerShape(8.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .border(1.dp, CardBorder, RoundedCornerShape(8.dp))
            ) {
                Column(modifier = Modifier.padding(14.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text("🖼️ Host Cover Image", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 15.sp)
                        Spacer(modifier = Modifier.weight(1f))
                        OutlinedButton(
                            onClick = { pickHostImageLauncher.launch("image/*") },
                            colors = ButtonDefaults.outlinedButtonColors(contentColor = AccentCyan),
                            border = ButtonDefaults.outlinedButtonBorder.copy(brush = Brush.linearGradient(listOf(CardBorder, CardBorder))),
                            shape = RoundedCornerShape(6.dp)
                        ) {
                            Text("Browse...")
                        }
                    }
                    if (hostImageName != null) {
                        Spacer(modifier = Modifier.height(6.dp))
                        Text(hostImageName!!, color = AccentCyan, fontSize = 13.sp, fontWeight = FontWeight.Medium)
                    }
                }
            }

            Spacer(modifier = Modifier.height(12.dp))

            // Payload Video Card
            Card(
                colors = CardDefaults.cardColors(containerColor = CardBg),
                shape = RoundedCornerShape(8.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .border(1.dp, CardBorder, RoundedCornerShape(8.dp))
            ) {
                Column(modifier = Modifier.padding(14.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text("🎥 Secret Video File", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 15.sp)
                        Spacer(modifier = Modifier.weight(1f))
                        OutlinedButton(
                            onClick = { pickPayloadVideoLauncher.launch("video/*") },
                            colors = ButtonDefaults.outlinedButtonColors(contentColor = AccentCyan),
                            border = ButtonDefaults.outlinedButtonBorder.copy(brush = Brush.linearGradient(listOf(CardBorder, CardBorder))),
                            shape = RoundedCornerShape(6.dp)
                        ) {
                            Text("Browse...")
                        }
                    }
                    if (payloadVideoName != null) {
                        Spacer(modifier = Modifier.height(6.dp))
                        Text(payloadVideoName!!, color = Color(0xFF818CF8), fontSize = 13.sp, fontWeight = FontWeight.Medium)
                    }
                }
            }

            Spacer(modifier = Modifier.height(12.dp))

            // Security Options Card
            Card(
                colors = CardDefaults.cardColors(containerColor = CardBg),
                shape = RoundedCornerShape(8.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .border(1.dp, CardBorder, RoundedCornerShape(8.dp))
            ) {
                Column(modifier = Modifier.padding(14.dp)) {
                    Text("🔐 Security & Password", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 15.sp)
                    Spacer(modifier = Modifier.height(8.dp))

                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Checkbox(
                            checked = enableEncryption,
                            onCheckedChange = { enableEncryption = it },
                            colors = CheckboxDefaults.colors(checkedColor = AccentCyan, checkmarkColor = Color.Black)
                        )
                        Text("Enable Password Encryption", color = Color.White, fontSize = 13.sp)
                    }

                    AnimatedVisibility(visible = enableEncryption) {
                        OutlinedTextField(
                            value = embedPassword,
                            onValueChange = { embedPassword = it },
                            label = { Text("Enter Password") },
                            visualTransformation = if (showEmbedPassword) VisualTransformation.None else PasswordVisualTransformation(),
                            trailingIcon = {
                                IconButton(onClick = { showEmbedPassword = !showEmbedPassword }) {
                                    Text(if (showEmbedPassword) "👁️" else "🔒", fontSize = 14.sp)
                                }
                            },
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(top = 8.dp),
                            shape = RoundedCornerShape(6.dp),
                            colors = OutlinedTextFieldDefaults.colors(
                                focusedBorderColor = AccentCyan,
                                unfocusedBorderColor = CardBorder,
                                focusedTextColor = Color.White,
                                unfocusedTextColor = Color.White
                            )
                        )
                    }
                }
            }

            Spacer(modifier = Modifier.height(16.dp))

            // Action Button
            val canEmbed = !isProcessing && hostImageUri != null && payloadVideoUri != null && (!enableEncryption || embedPassword.isNotEmpty())
            Button(
                onClick = { saveCarrierLauncher.launch(hostImageName?.substringBeforeLast('.') + "_carrier.png") },
                enabled = canEmbed,
                modifier = Modifier
                    .fillMaxWidth()
                    .height(48.dp),
                shape = RoundedCornerShape(8.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = AccentBlue,
                    disabledContainerColor = Color(0xFF1E293B),
                    contentColor = Color.White,
                    disabledContentColor = Color.Gray
                )
            ) {
                Text(
                    text = if (isProcessing) "⏳ Streaming Carrier..." else "🚀 Embed & Conceal Video into Image",
                    fontWeight = FontWeight.Bold,
                    fontSize = 15.sp
                )
            }
        }

        // TAB 2: EXTRACT
        if (selectedTab == 1) {
            Card(
                colors = CardDefaults.cardColors(containerColor = CardBg),
                shape = RoundedCornerShape(8.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .border(1.dp, CardBorder, RoundedCornerShape(8.dp))
            ) {
                Column(modifier = Modifier.padding(14.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text("🖼️ Select Carrier Image", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 15.sp)
                        Spacer(modifier = Modifier.weight(1f))
                        OutlinedButton(
                            onClick = { pickCarrierLauncher.launch("image/*") },
                            colors = ButtonDefaults.outlinedButtonColors(contentColor = AccentCyan),
                            border = ButtonDefaults.outlinedButtonBorder.copy(brush = Brush.linearGradient(listOf(CardBorder, CardBorder))),
                            shape = RoundedCornerShape(6.dp)
                        ) {
                            Text("Browse...")
                        }
                    }
                    if (carrierName != null) {
                        Spacer(modifier = Modifier.height(6.dp))
                        Text(carrierName!!, color = AccentCyan, fontSize = 13.sp, fontWeight = FontWeight.Medium)
                    }
                }
            }

            Spacer(modifier = Modifier.height(12.dp))

            inspectedInfo?.let { info ->
                if (info.isEncrypted) {
                    Card(
                        colors = CardDefaults.cardColors(containerColor = CardBg),
                        shape = RoundedCornerShape(8.dp),
                        modifier = Modifier
                            .fillMaxWidth()
                            .border(1.dp, CardBorder, RoundedCornerShape(8.dp))
                    ) {
                        Column(modifier = Modifier.padding(14.dp)) {
                            Text("🔐 Password Required", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 15.sp)
                            Spacer(modifier = Modifier.height(6.dp))
                            OutlinedTextField(
                                value = extractPassword,
                                onValueChange = { extractPassword = it },
                                label = { Text("Password") },
                                visualTransformation = if (showExtractPassword) VisualTransformation.None else PasswordVisualTransformation(),
                                trailingIcon = {
                                    IconButton(onClick = { showExtractPassword = !showExtractPassword }) {
                                        Text(if (showExtractPassword) "👁️" else "🔒", fontSize = 14.sp)
                                    }
                                },
                                modifier = Modifier.fillMaxWidth(),
                                shape = RoundedCornerShape(6.dp),
                                colors = OutlinedTextFieldDefaults.colors(
                                    focusedBorderColor = AccentCyan,
                                    unfocusedBorderColor = CardBorder,
                                    focusedTextColor = Color.White,
                                    unfocusedTextColor = Color.White
                                )
                            )
                        }
                    }
                    Spacer(modifier = Modifier.height(12.dp))
                }
            }

            // Extract Action Button
            val canExtract = !isProcessing && carrierUri != null
            Button(
                onClick = {
                    val defaultName = inspectedInfo?.originalFilename ?: "extracted_video.mp4"
                    saveExtractedLauncher.launch(defaultName)
                },
                enabled = canExtract,
                modifier = Modifier
                    .fillMaxWidth()
                    .height(48.dp),
                shape = RoundedCornerShape(8.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = EmeraldSuccess,
                    disabledContainerColor = Color(0xFF1E293B),
                    contentColor = Color.White,
                    disabledContentColor = Color.Gray
                )
            ) {
                Text(
                    text = if (isProcessing) "⏳ Extracting..." else "🔓 Extract Hidden Video",
                    fontWeight = FontWeight.Bold,
                    fontSize = 15.sp
                )
            }
        }

        // TAB 3: INSPECT
        if (selectedTab == 2) {
            Card(
                colors = CardDefaults.cardColors(containerColor = CardBg),
                shape = RoundedCornerShape(8.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .border(1.dp, CardBorder, RoundedCornerShape(8.dp))
            ) {
                Column(modifier = Modifier.padding(14.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text("🔍 Inspect Image", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 15.sp)
                        Spacer(modifier = Modifier.weight(1f))
                        OutlinedButton(
                            onClick = { pickCarrierLauncher.launch("image/*") },
                            colors = ButtonDefaults.outlinedButtonColors(contentColor = AccentCyan),
                            border = ButtonDefaults.outlinedButtonBorder.copy(brush = Brush.linearGradient(listOf(CardBorder, CardBorder))),
                            shape = RoundedCornerShape(6.dp)
                        ) {
                            Text("Browse...")
                        }
                    }
                }
            }

            Spacer(modifier = Modifier.height(12.dp))

            inspectedInfo?.let { info ->
                Card(
                    colors = CardDefaults.cardColors(containerColor = DarkNavy),
                    shape = RoundedCornerShape(8.dp),
                    modifier = Modifier
                        .fillMaxWidth()
                        .border(1.dp, EmeraldSuccess, RoundedCornerShape(8.dp))
                ) {
                    Column(modifier = Modifier.padding(14.dp)) {
                        Text("✅ Valid Secret PNG Carrier Detected", color = EmeraldSuccess, fontWeight = FontWeight.Bold, fontSize = 15.sp)
                        Spacer(modifier = Modifier.height(6.dp))
                        Text("• Original Name: ${info.originalFilename}", color = Color.White, fontSize = 13.sp)
                        Text("• Video Size: ${formatBytes(info.payloadSize)}", color = Color.White, fontSize = 13.sp)
                        Text("• Encrypted: ${if (info.isEncrypted) "Yes (Password Protected)" else "No"}", color = Color.White, fontSize = 13.sp)
                        Text("• Checksum: ${info.sha256Hex}", color = TextMuted, fontSize = 11.sp)
                    }
                }
            }
        }

        // Report summary
        lastReport?.let { report ->
            Spacer(modifier = Modifier.height(16.dp))
            Card(
                colors = CardDefaults.cardColors(containerColor = DarkNavy),
                shape = RoundedCornerShape(8.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .border(1.dp, EmeraldSuccess, RoundedCornerShape(8.dp))
            ) {
                Column(modifier = Modifier.padding(14.dp)) {
                    Text("🎉 Operation Report", color = EmeraldSuccess, fontWeight = FontWeight.Bold, fontSize = 15.sp)
                    Spacer(modifier = Modifier.height(4.dp))
                    Text("• File: ${report.originalFilename}", color = Color.White, fontSize = 13.sp)
                    Text("• Size: ${formatBytes(report.payloadSize)}", color = Color.White, fontSize = 13.sp)
                    Text("• SHA-256 / BLAKE3: ${report.sha256Hex}", color = TextMuted, fontSize = 11.sp)
                }
            }
        }
    }
}
