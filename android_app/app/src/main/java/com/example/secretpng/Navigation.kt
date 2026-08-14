package com.example.secretpng

import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.navigation3.runtime.entryProvider
import androidx.navigation3.runtime.rememberNavBackStack
import androidx.navigation3.ui.NavDisplay
import com.example.secretpng.ui.main.MainScreen

@Composable
fun MainNavigation() {
  MainScreen(modifier = Modifier.safeDrawingPadding())
}
