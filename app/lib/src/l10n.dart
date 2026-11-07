import 'dart:ui' show Locale, PlatformDispatcher;

import 'package:intl/date_symbol_data_local.dart';
import 'package:intl/intl.dart';

import '../l10n/app_localizations.dart';
import 'theme.dart' show useFontsFor;

export '../l10n/app_localizations.dart' show L10n;

/// The languages Fogata's own words come in, by code, each named in itself.
const languages = {
  'en': 'English',
  'es': 'Español',
  'pt-BR': 'Português do Brasil',
  'fr': 'Français',
  'de': 'Deutsch',
  'it': 'Italiano',
  'ru': 'Русский',
  'ja': '日本語',
  'zh-Hans': '简体中文',
};

/// The locale of one of [languages]. `app_pt.arb` holds Brazilian Portuguese and
/// `app_zh.arb` Simplified Chinese.
Locale localeOf(String code) => switch (code) {
  'pt-BR' => const Locale('pt', 'BR'),
  'zh-Hans' => const Locale.fromSubtags(languageCode: 'zh', scriptCode: 'Hans'),
  _ => Locale(code),
};

/// The first of the system's languages Fogata has (any Portuguese is Brazilian, Chinese
/// written in simplified characters is Simplified Chinese), or English.
String systemLanguage([List<Locale>? system]) {
  for (final locale in system ?? PlatformDispatcher.instance.locales) {
    final code = switch (locale.languageCode) {
      'pt' => 'pt-BR',
      'zh' when _simplified(locale) => 'zh-Hans',
      'zh' => null,
      final other when languages.containsKey(other) => other,
      _ => null,
    };
    if (code != null) return code;
  }
  return 'en';
}

bool _simplified(Locale locale) => switch (locale.scriptCode) {
  'Hans' => true,
  'Hant' => false,
  _ => switch (locale.countryCode) {
    null || 'CN' || 'SG' => true,
    _ => false,
  },
};

/// The words of the language in use, for code that has no [BuildContext]; the window is
/// built again whole when the language changes.
L10n l10n = lookupL10n(const Locale('en'));

/// The code of the language in use, one of [languages].
String currentLanguage = 'en';

/// The locale `intl` formats dates and numbers in for [currentLanguage], its data
/// loaded.
String get intlLocale {
  _dates ??= initializeDateFormatting();
  return switch (currentLanguage) {
    'pt-BR' => 'pt_BR',
    'zh-Hans' => 'zh',
    final code => code,
  };
}

/// Uses [code] (one of [languages]) for every word said from now on.
void useLanguage(String code) {
  currentLanguage = languages.containsKey(code) ? code : 'en';
  l10n = lookupL10n(localeOf(currentLanguage));
  Intl.defaultLocale = intlLocale;
  useFontsFor(currentLanguage);
}

Future<void>? _dates;
