// What Fogata Updater requires of a new version before it puts it in place of the installed
// one (main.swift), apart from the arguments: kept in a file of its own so that
// tools/beta/test-macos-updater.sh can try it on bundles signed for the test.

import Foundation
import Security

enum Checks {
  /// Fogata's identifier, and the App Sandbox: debug and profile builds, signed by the same
  /// team but run outside the sandbox (app/macos/Runner/DebugProfile.entitlements), are not
  /// versions to install.
  static let fogataRequirement =
    "identifier \"dev.holypanda.fogata\" and entitlement[\"com.apple.security.app-sandbox\"] exists"

  /// The most builds a new version may be above the installed one: a release of the rescue
  /// key may go that far (MAX_RESCUE_STEP in crates/fogata-update/src/lib.rs), any other less.
  static let maxBuildStep: UInt64 = 2000

  /// The requirement a new version's signature must meet: Apple's chain, the team, Fogata.
  static func requirement(team: String) -> String {
    "anchor apple generic and certificate leaf[subject.OU] = \"\(team)\" and \(fogataRequirement)"
  }

  /// A build number as Fogata's builds carry it in CFBundleVersion: digits only.
  static func build(_ value: Any?) -> UInt64? {
    guard let text = value as? String, !text.isEmpty, text.count <= 18,
      text.allSatisfy({ $0.isASCII && $0.isNumber })
    else { return nil }
    return UInt64(text)
  }

  /// The Info.plist and the entitlements the code at `code` was signed with.
  static func signed(_ code: SecStaticCode) -> (plist: [String: Any], entitlements: [String: Any])? {
    var info: CFDictionary?
    let flags = SecCSFlags(rawValue: kSecCSSigningInformation | kSecCSRequirementInformation)
    guard SecCodeCopySigningInformation(code, flags, &info) == errSecSuccess,
      let dictionary = info as? [String: Any],
      let plist = dictionary[kSecCodeInfoPList as String] as? [String: Any]
    else { return nil }
    let entitlements = dictionary[kSecCodeInfoEntitlementsDict as String] as? [String: Any] ?? [:]
    return (plist, entitlements)
  }

  /// The build of the installed bundle, as its signed Info.plist says.
  static func installedBuild(_ installed: URL) -> UInt64? {
    var code: SecStaticCode?
    guard SecStaticCodeCreateWithPath(installed as CFURL, [], &code) == errSecSuccess, let code,
      let (plist, _) = signed(code)
    else { return nil }
    return build(plist["CFBundleVersion"])
  }

  /// Why the bundle at `copy` may not replace the installed one, or nil if it may: its
  /// signature must be valid and meet `requirement`, it must run in the App Sandbox, and its
  /// build, as the Info.plist its signature seals says, must be above the installed build
  /// (never the same or older: the build rules Fogata applies before it downloads a release
  /// hold here too, whoever put the bundle where the helper takes it) and at most
  /// `maxBuildStep` above it.
  static func refusal(newVersion copy: URL, installedBuild: UInt64, requirement text: String)
    -> String?
  {
    var requirement: SecRequirement?
    guard SecRequirementCreateWithString(text as CFString, [], &requirement) == errSecSuccess,
      let requirement
    else { return "cannot make the signing requirement" }
    var code: SecStaticCode?
    guard SecStaticCodeCreateWithPath(copy as CFURL, [], &code) == errSecSuccess, let code else {
      return "the new version is not code"
    }
    let flags = SecCSFlags(
      rawValue: kSecCSCheckAllArchitectures | kSecCSCheckNestedCode | kSecCSStrictValidate)
    let checked = SecStaticCodeCheckValidity(code, flags, requirement)
    guard checked == errSecSuccess else {
      return "the new version is not a sandboxed Fogata signed as required (\(checked))"
    }
    guard let (plist, entitlements) = signed(code) else {
      return "cannot read what the new version's signature seals"
    }
    guard entitlements["com.apple.security.app-sandbox"] as? Bool == true else {
      return "the new version does not run in the App Sandbox"
    }
    guard let newBuild = build(plist["CFBundleVersion"]) else {
      return "the new version carries no build number"
    }
    guard newBuild > installedBuild else {
      return "the new version is build \(newBuild), not newer than the installed build \(installedBuild)"
    }
    guard newBuild - installedBuild <= maxBuildStep else {
      return "the new version is build \(newBuild), more than \(maxBuildStep) above the installed build \(installedBuild)"
    }
    return nil
  }
}
