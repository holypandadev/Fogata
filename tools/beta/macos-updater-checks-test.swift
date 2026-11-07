// Tries Checks.refusal (app/macos/Updater/Checks.swift) on bundles that
// tools/beta/test-macos-updater.sh signed for the test (ad hoc, so Apple's chain and the team
// are left out of the requirement): arguments <folder of bundles> <installed build>. Each
// bundle is named <what it is>-<yes|no>.app, yes when the helper may install it.

import Foundation

let arguments = CommandLine.arguments
guard arguments.count == 3, let installed = UInt64(arguments[2]) else {
  FileHandle.standardError.write(Data("usage: <folder> <installed build>\n".utf8))
  exit(2)
}
let folder = URL(fileURLWithPath: arguments[1])
var failed = 0
let names = try FileManager.default.contentsOfDirectory(atPath: folder.path).sorted()
for name in names where name.hasSuffix("-yes.app") || name.hasSuffix("-no.app") {
  let expected = name.hasSuffix("-yes.app")
  let why = Checks.refusal(
    newVersion: folder.appendingPathComponent(name), installedBuild: installed,
    requirement: Checks.fogataRequirement)
  if (why == nil) != expected {
    print("FAIL: \(name): \(why ?? "taken")")
    failed += 1
  } else {
    print("ok: \(name): \(why ?? "taken")")
  }
}
// The release's requirement parses, and a bundle signed ad hoc does not meet it.
let underTeam = Checks.refusal(
  newVersion: folder.appendingPathComponent("newer-yes.app"), installedBuild: installed,
  requirement: Checks.requirement(team: "ABCDE12345"))
if underTeam == nil || underTeam == "cannot make the signing requirement" {
  print("FAIL: the release's requirement: \(underTeam ?? "taken")")
  failed += 1
}
if Checks.installedBuild(folder.appendingPathComponent("installed.app")) != installed {
  print("FAIL: the installed bundle's build is not read as \(installed)")
  failed += 1
}
for (text, build) in [("7", UInt64(7)), ("0012", 12), ("", nil), ("1.2", nil), ("-3", nil),
  ("٣", nil), ("99999999999999999999", nil)] as [(String, UInt64?)]
{
  if Checks.build(text) != build {
    print("FAIL: build \"\(text)\" read as \(String(describing: Checks.build(text)))")
    failed += 1
  }
}
if Checks.build(7) != nil {
  print("FAIL: a number that is not a string read as a build")
  failed += 1
}
exit(failed == 0 ? 0 : 1)
