import 'package:flutter/material.dart';

import 'rally_tokens.dart';

/// 生徒アプリのテーマ。specs/design/tokens.yaml の色・書体・余白・角丸から作る（影は使わない）。
/// 明るい配色だけで、端末がダークでもこのテーマで出す（color.dark_mode.gen1）。
ThemeData buildRallyTheme() {
  final scheme = ColorScheme(
    brightness: Brightness.light,
    primary: RallyColors.primary,
    onPrimary: RallyColors.onPrimary,
    primaryContainer: RallyColors.primaryContainer,
    onPrimaryContainer: RallyColors.onPrimaryContainer,
    secondary: RallyColors.secondary,
    onSecondary: RallyColors.onSecondary,
    secondaryContainer: RallyColors.primaryContainer,
    onSecondaryContainer: RallyColors.onPrimaryContainer,
    tertiary: RallyColors.primary,
    onTertiary: RallyColors.onPrimary,
    error: RallyColors.danger.fg,
    onError: RallyColors.onPrimary,
    errorContainer: RallyColors.danger.container,
    onErrorContainer: RallyColors.danger.onContainer,
    surface: RallyColors.background,
    onSurface: RallyColors.text,
    onSurfaceVariant: RallyColors.textMuted,
    surfaceContainerLowest: RallyColors.background,
    surfaceContainerLow: RallyColors.surface,
    surfaceContainer: RallyColors.surface,
    surfaceContainerHigh: RallyColors.surface,
    surfaceContainerHighest: RallyColors.surface,
    outline: RallyColors.borderStrong,
    outlineVariant: RallyColors.border,
    inverseSurface: RallyColors.inverseSurface,
    onInverseSurface: RallyColors.onInverseSurface,
    inversePrimary: RallyColors.secondary,
    scrim: RallyColors.scrim,
    shadow: Colors.transparent,
    surfaceTint: Colors.transparent,
  );

  final textTheme = TextTheme(
    displayLarge: RallyTypography.display,
    displayMedium: RallyTypography.display,
    displaySmall: RallyTypography.display,
    headlineLarge: RallyTypography.headline,
    headlineMedium: RallyTypography.headline,
    headlineSmall: RallyTypography.headline,
    titleLarge: RallyTypography.title,
    titleMedium: RallyTypography.title,
    titleSmall: RallyTypography.body.copyWith(fontWeight: FontWeight.w700),
    bodyLarge: RallyTypography.body,
    bodyMedium: RallyTypography.body,
    bodySmall: RallyTypography.small,
    labelLarge: RallyTypography.label,
    labelMedium: RallyTypography.small.copyWith(fontWeight: FontWeight.w700),
    labelSmall: RallyTypography.caption,
  ).apply(bodyColor: RallyColors.text, displayColor: RallyColors.text);

  const pill = StadiumBorder();
  final md = RoundedRectangleBorder(
    borderRadius: BorderRadius.circular(RallyRadius.md),
  );
  const buttonPadding = EdgeInsets.symmetric(horizontal: RallySpacing.lg);
  const buttonSize = Size(RallySpacing.touchTarget, RallySpacing.touchTarget);

  final primaryButton = ButtonStyle(
    shape: const WidgetStatePropertyAll(pill),
    minimumSize: const WidgetStatePropertyAll(buttonSize),
    padding: const WidgetStatePropertyAll(buttonPadding),
    textStyle: WidgetStatePropertyAll(RallyTypography.label),
    elevation: const WidgetStatePropertyAll(0),
    animationDuration: RallyMotion.fast,
    backgroundColor: WidgetStateProperty.resolveWith((states) {
      if (states.contains(WidgetState.disabled)) return RallyColors.surface;
      if (states.contains(WidgetState.pressed)) {
        return RallyColors.primaryPressed;
      }
      return RallyColors.primary;
    }),
    foregroundColor: WidgetStateProperty.resolveWith(
      (states) => states.contains(WidgetState.disabled)
          ? RallyColors.textMuted
          : RallyColors.onPrimary,
    ),
    overlayColor: const WidgetStatePropertyAll(Colors.transparent),
  );

  OutlineInputBorder inputBorder(Color color, double width) =>
      OutlineInputBorder(
        borderRadius: BorderRadius.circular(RallyRadius.md),
        borderSide: BorderSide(color: color, width: width),
      );

  return ThemeData(
    useMaterial3: true,
    colorScheme: scheme,
    scaffoldBackgroundColor: RallyColors.background,
    fontFamily: RallyTypography.family,
    fontFamilyFallback: RallyTypography.fallback,
    textTheme: textTheme,
    visualDensity: VisualDensity.standard,
    materialTapTargetSize: MaterialTapTargetSize.padded,
    splashFactory: NoSplash.splashFactory,
    dividerTheme: const DividerThemeData(
      color: RallyColors.border,
      thickness: 1,
      space: 1,
    ),
    appBarTheme: AppBarTheme(
      backgroundColor: RallyColors.background,
      foregroundColor: RallyColors.text,
      surfaceTintColor: Colors.transparent,
      elevation: 0,
      scrolledUnderElevation: 0,
      shape: const Border(bottom: BorderSide(color: RallyColors.border)),
      titleTextStyle: RallyTypography.title.copyWith(color: RallyColors.text),
    ),
    filledButtonTheme: FilledButtonThemeData(style: primaryButton),
    elevatedButtonTheme: ElevatedButtonThemeData(style: primaryButton),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: ButtonStyle(
        shape: const WidgetStatePropertyAll(pill),
        minimumSize: const WidgetStatePropertyAll(buttonSize),
        padding: const WidgetStatePropertyAll(buttonPadding),
        textStyle: WidgetStatePropertyAll(RallyTypography.label),
        animationDuration: RallyMotion.fast,
        backgroundColor: WidgetStateProperty.resolveWith((states) {
          if (states.contains(WidgetState.disabled)) return RallyColors.surface;
          if (states.contains(WidgetState.pressed)) {
            return RallyColors.primaryContainer;
          }
          return RallyColors.background;
        }),
        foregroundColor: WidgetStateProperty.resolveWith(
          (states) => states.contains(WidgetState.disabled)
              ? RallyColors.textMuted
              : RallyColors.primary,
        ),
        side: WidgetStateProperty.resolveWith(
          (states) => states.contains(WidgetState.disabled)
              ? BorderSide.none
              : const BorderSide(color: RallyColors.borderStrong),
        ),
        overlayColor: const WidgetStatePropertyAll(Colors.transparent),
      ),
    ),
    textButtonTheme: TextButtonThemeData(
      style: ButtonStyle(
        shape: const WidgetStatePropertyAll(pill),
        minimumSize: const WidgetStatePropertyAll(buttonSize),
        padding: const WidgetStatePropertyAll(
          EdgeInsets.symmetric(horizontal: RallySpacing.md),
        ),
        textStyle: WidgetStatePropertyAll(RallyTypography.label),
        animationDuration: RallyMotion.fast,
        backgroundColor: WidgetStateProperty.resolveWith(
          (states) => states.contains(WidgetState.pressed)
              ? RallyColors.primaryContainer
              : null,
        ),
        foregroundColor: WidgetStateProperty.resolveWith(
          (states) => states.contains(WidgetState.disabled)
              ? RallyColors.textMuted
              : RallyColors.primary,
        ),
        overlayColor: const WidgetStatePropertyAll(Colors.transparent),
      ),
    ),
    iconButtonTheme: const IconButtonThemeData(
      style: ButtonStyle(minimumSize: WidgetStatePropertyAll(buttonSize)),
    ),
    inputDecorationTheme: InputDecorationTheme(
      filled: true,
      fillColor: RallyColors.background,
      contentPadding: const EdgeInsets.symmetric(
        horizontal: RallySpacing.md,
        vertical: RallySpacing.md,
      ),
      labelStyle: RallyTypography.small.copyWith(
        fontWeight: FontWeight.w700,
        color: RallyColors.text,
      ),
      floatingLabelStyle: RallyTypography.small.copyWith(
        fontWeight: FontWeight.w700,
        color: RallyColors.text,
      ),
      helperStyle: RallyTypography.small.copyWith(color: RallyColors.textMuted),
      counterStyle: RallyTypography.small.copyWith(
        color: RallyColors.textMuted,
      ),
      errorStyle: RallyTypography.small.copyWith(color: RallyColors.danger.fg),
      errorMaxLines: 3,
      border: inputBorder(RallyColors.borderStrong, 1),
      enabledBorder: inputBorder(RallyColors.borderStrong, 1),
      focusedBorder: inputBorder(RallyColors.focusRing, 2),
      errorBorder: inputBorder(RallyColors.danger.fg, 2),
      focusedErrorBorder: inputBorder(RallyColors.danger.fg, 2),
      disabledBorder: inputBorder(RallyColors.border, 1),
    ),
    cardTheme: CardThemeData(
      color: RallyColors.surface,
      surfaceTintColor: Colors.transparent,
      shadowColor: Colors.transparent,
      elevation: 0,
      margin: const EdgeInsets.symmetric(vertical: RallySpacing.xs),
      shape: md,
    ),
    listTileTheme: ListTileThemeData(
      minTileHeight: 56,
      contentPadding: const EdgeInsets.symmetric(horizontal: RallySpacing.md),
      titleTextStyle: RallyTypography.body.copyWith(
        fontWeight: FontWeight.w700,
        color: RallyColors.text,
      ),
      subtitleTextStyle: RallyTypography.small.copyWith(
        color: RallyColors.textMuted,
      ),
      iconColor: RallyColors.text,
      selectedColor: RallyColors.onPrimaryContainer,
      selectedTileColor: RallyColors.primaryContainer,
    ),
    chipTheme: ChipThemeData(
      shape: pill,
      side: const BorderSide(color: RallyColors.borderStrong),
      backgroundColor: RallyColors.background,
      selectedColor: RallyColors.primaryContainer,
      labelStyle: RallyTypography.small.copyWith(color: RallyColors.text),
      secondaryLabelStyle: RallyTypography.small.copyWith(
        fontWeight: FontWeight.w700,
        color: RallyColors.onPrimaryContainer,
      ),
      checkmarkColor: RallyColors.onPrimaryContainer,
      elevation: 0,
      pressElevation: 0,
    ),
    badgeTheme: BadgeThemeData(
      backgroundColor: RallyColors.danger.fg,
      textColor: RallyColors.onPrimary,
      textStyle: RallyTypography.caption,
      smallSize: 8,
      largeSize: 20,
    ),
    progressIndicatorTheme: const ProgressIndicatorThemeData(
      color: RallyColors.primary,
      linearTrackColor: RallyColors.primaryContainer,
      linearMinHeight: 4,
    ),
    dialogTheme: DialogThemeData(
      backgroundColor: RallyColors.background,
      surfaceTintColor: Colors.transparent,
      elevation: 0,
      shape: md,
      barrierColor: RallyColors.scrim,
      titleTextStyle: RallyTypography.title.copyWith(color: RallyColors.text),
      contentTextStyle: RallyTypography.body.copyWith(color: RallyColors.text),
    ),
    snackBarTheme: SnackBarThemeData(
      backgroundColor: RallyColors.inverseSurface,
      contentTextStyle: RallyTypography.body.copyWith(
        color: RallyColors.onInverseSurface,
      ),
      behavior: SnackBarBehavior.floating,
      elevation: 0,
      shape: md,
    ),
    switchTheme: SwitchThemeData(
      trackOutlineColor: WidgetStateProperty.resolveWith(
        (states) => states.contains(WidgetState.selected)
            ? RallyColors.primary
            : RallyColors.borderStrong,
      ),
    ),
    segmentedButtonTheme: SegmentedButtonThemeData(
      style: ButtonStyle(
        minimumSize: const WidgetStatePropertyAll(buttonSize),
        side: const WidgetStatePropertyAll(
          BorderSide(color: RallyColors.borderStrong),
        ),
        backgroundColor: WidgetStateProperty.resolveWith(
          (states) => states.contains(WidgetState.selected)
              ? RallyColors.primaryContainer
              : RallyColors.background,
        ),
        foregroundColor: WidgetStateProperty.resolveWith(
          (states) => states.contains(WidgetState.selected)
              ? RallyColors.onPrimaryContainer
              : RallyColors.text,
        ),
      ),
    ),
    pageTransitionsTheme: const PageTransitionsTheme(),
  );
}
