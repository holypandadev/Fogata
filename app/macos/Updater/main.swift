// Fogata Updater: puts a new version of Fogata in place of the one that opened it. Fogata
// runs in the App Sandbox and cannot replace its own bundle; this helper, inside it
// (Contents/Library/Helpers) and signed by the same team without the sandbox, is opened by
// Launch Services as an application of its own.
//
// Arguments: <pid of Fogata> <new Fogata.app>
//
// Anyone can open the helper with arguments of their choosing, so it believes them only as
// far as it can check them. The installed bundle is the one this helper is inside of, never
// an argument. The new bundle must be at the one place the sandboxed Fogata unpacks it,
// inside Fogata's own container (Pinned.container under the home of the user running), with
// no link on the way, as it is told and again once Fogata has quit; the pid must be a running
// process of the installed bundle. The new bundle is first copied to a folder of the helper's
// own that nobody else can write, and it is that copy, a folder and not a link, that is
// checked (a valid signature, for Fogata's identifier, by the team pinned in at build time
// or, failing that, this helper's own), cleared of the quarantine the sandbox put on it, and
// swapped in; the bundle at the place the arguments named is never touched again after the
// copy, so what was checked is what is installed. That copy must also run in the App Sandbox
// (no debug or profile build) and be a newer build than the installed one, as the Info.plist
// its signature seals says (Checks.swift): Fogata applies the build rules of a release before
// it downloads it, but anything that can write Fogata's container, Fogata included, could put
// an older bundle of the same team there. If anything fails before the swap, the installed
// version is opened again unchanged.

import AppKit
import Darwin
import Foundation
import Security

func say(_ text: String) {
  FileHandle.standardError.write(Data(("Fogata Updater: " + text + "\n").utf8))
}

let files = FileManager.default

/// Refuses the arguments: nothing was touched.
func refuse(_ why: String) -> Never {
  say(why)
  exit(2)
}

/// The path with every link resolved, or nil if it is not there.
func real(_ path: String) -> String? {
  guard let resolved = realpath(path, nil) else { return nil }
  defer { free(resolved) }
  return String(cString: resolved)
}

/// Whether `path` is `root` or inside it (both resolved).
func inside(_ path: String, _ root: String) -> Bool {
  path == root || path.hasPrefix(root.hasSuffix("/") ? root : root + "/")
}

// The installed bundle: four levels above this helper
// (Fogata.app/Contents/Library/Helpers/Fogata Updater.app).
guard let helperPath = real(Bundle.main.bundleURL.path) else {
  refuse("cannot tell where this helper is")
}
let target = URL(fileURLWithPath: helperPath)
  .deletingLastPathComponent().deletingLastPathComponent()
  .deletingLastPathComponent().deletingLastPathComponent()
guard target.pathExtension == "app",
  real(target.appendingPathComponent("Contents/Library/Helpers/Fogata Updater.app").path)
    == helperPath,
  files.fileExists(atPath: target.appendingPathComponent("Contents/MacOS/Fogata").path)
else {
  refuse("this helper is not inside an installed Fogata.app")
}

let arguments = CommandLine.arguments
guard arguments.count == 3, let pid = pid_t(arguments[1]), pid > 0 else {
  refuse("usage: <pid> <new Fogata.app>")
}

// The new bundle: inside Fogata's container, at the place Fogata unpacks it, no link on
// the way (the path as given must be the path as resolved).
let home: String = {
  if let entry = getpwuid(getuid()), let dir = entry.pointee.pw_dir {
    return String(cString: dir)
  }
  return NSHomeDirectory()
}()
let containerRoot =
  Pinned.container.hasPrefix("/") ? Pinned.container : home + "/" + Pinned.container
guard let realContainer = real(containerRoot) else {
  refuse("Fogata's container \(containerRoot) is not there")
}
let stagedGiven = URL(fileURLWithPath: arguments[2]).standardizedFileURL.path

/// Whether the new bundle is at Fogata's place for it: a folder, as given is as resolved (no
/// link anywhere on the way, nothing relative). Checked again once Fogata has quit, since
/// until then Fogata can still change what is there.
func stagedInPlace() -> Bool {
  guard let stagedReal = real(stagedGiven) else { return false }
  return stagedReal == stagedGiven
    && inside(stagedReal, realContainer) && stagedReal != realContainer
    && stagedReal.hasSuffix("/updates/Fogata.app")
    && (try? files.attributesOfItem(atPath: stagedReal))?[.type] as? FileAttributeType
      == .typeDirectory
}
guard stagedInPlace() else {
  refuse("the new version is not at Fogata's place for it")
}
let staged = URL(fileURLWithPath: stagedGiven)

// The pid: a running process of the installed bundle.
var pathBuffer = [CChar](repeating: 0, count: Int(PATH_MAX) * 4)
let pathLength = proc_pidpath(pid, &pathBuffer, UInt32(pathBuffer.count))
guard pathLength > 0, let processPath = real(String(cString: pathBuffer)),
  let targetReal = real(target.path), inside(processPath, targetReal)
else {
  refuse("process \(pid) is not the installed Fogata")
}

/// Opens `app` as a new instance, with this helper's FOGATA_* settings (a test copy's).
func open(_ app: URL) {
  let configuration = NSWorkspace.OpenConfiguration()
  configuration.createsNewApplicationInstance = true
  configuration.environment = ProcessInfo.processInfo.environment.filter {
    $0.key.hasPrefix("FOGATA_")
  }
  let done = DispatchSemaphore(value: 0)
  NSWorkspace.shared.openApplication(at: app, configuration: configuration) { _, error in
    if let error = error { say("cannot open \(app.path): \(error.localizedDescription)") }
    done.signal()
  }
  _ = done.wait(timeout: .now() + 30)
}

/// A folder of the helper's own on the installed bundle's volume, which nobody else can
/// write: the new bundle is checked and installed from a copy there.
func privateFolder() -> URL? {
  guard
    let folder = try? files.url(
      for: .itemReplacementDirectory, in: .userDomainMask, appropriateFor: target,
      create: true)
  else { return nil }
  guard (try? files.setAttributes([.posixPermissions: 0o700], ofItemAtPath: folder.path)) != nil
  else {
    try? files.removeItem(at: folder)
    return nil
  }
  return folder
}

var scratch: URL?

/// Gives up after the arguments were taken: the new bundle goes, the installed version is
/// opened again unchanged.
func giveUp(_ why: String) -> Never {
  say(why)
  if let scratch { try? files.removeItem(at: scratch) }
  try? files.removeItem(at: staged)
  open(target)
  exit(1)
}

/// The team that signed the code at `url`.
func team(of url: URL) -> String? {
  var code: SecStaticCode?
  guard SecStaticCodeCreateWithPath(url as CFURL, [], &code) == errSecSuccess, let code else {
    return nil
  }
  var info: CFDictionary?
  guard
    SecCodeCopySigningInformation(code, SecCSFlags(rawValue: kSecCSSigningInformation), &info)
      == errSecSuccess,
    let dictionary = info as? [String: Any]
  else { return nil }
  return dictionary[kSecCodeInfoTeamIdentifier as String] as? String
}

// Fogata quits right after opening this helper; two minutes at most.
var waited = 0
while kill(pid, 0) == 0 && waited < 1200 {
  usleep(100_000)
  waited += 1
}
if kill(pid, 0) == 0 {
  say("Fogata did not quit; nothing changed")
  exit(1)
}
guard stagedInPlace() else {
  giveUp("the new version is not at Fogata's place for it")
}

guard let ownTeam = team(of: Bundle.main.bundleURL) else {
  giveUp("this helper is not signed by a team")
}
let requiredTeam = Pinned.teamID ?? ownTeam
guard requiredTeam == ownTeam else {
  giveUp("this helper is signed by team \(ownTeam), not \(requiredTeam)")
}

// A copy of the new bundle that only this helper can reach from now on.
guard let folder = privateFolder() else {
  giveUp("cannot make a private folder on the installed version's volume")
}
scratch = folder
let copy = folder.appendingPathComponent("Fogata.app")
do {
  try files.copyItem(at: staged, to: copy)
} catch {
  giveUp("cannot copy the new version: \(error.localizedDescription)")
}
// A link copies as a link, and its signature would be checked where it points: the copy
// must be the folder itself.
guard (try? files.attributesOfItem(atPath: copy.path))?[.type] as? FileAttributeType
  == .typeDirectory
else {
  giveUp("the new version is not a folder")
}

guard let installedBuild = Checks.installedBuild(target) else {
  giveUp("cannot tell the installed version's build")
}
if let why = Checks.refusal(
  newVersion: copy, installedBuild: installedBuild,
  requirement: Checks.requirement(team: requiredTeam))
{
  giveUp("\(why); team \(requiredTeam) required")
}

// Files the sandboxed application wrote are quarantined; this copy was checked above.
let xattr = Process()
xattr.executableURL = URL(fileURLWithPath: "/usr/bin/xattr")
xattr.arguments = ["-dr", "com.apple.quarantine", copy.path]
try? xattr.run()
xattr.waitUntilExit()

// Fogata unpacks the new bundle readable by its user only; installed, every user of this Mac
// opens it, as the bundle it replaces. Only the bundle's own folder is changed: what is inside
// it was unpacked readable by all, and permissions are not part of the signature checked.
do {
  try files.setAttributes([.posixPermissions: 0o755], ofItemAtPath: copy.path)
} catch {
  giveUp("cannot make the new version readable by every user: \(error.localizedDescription)")
}

let old = target.deletingLastPathComponent().appendingPathComponent(".Fogata.app.old")
try? files.removeItem(at: old)
do {
  try files.moveItem(at: target, to: old)
} catch {
  giveUp("cannot move the installed version aside: \(error.localizedDescription)")
}
do {
  try files.moveItem(at: copy, to: target)
} catch {
  try? files.moveItem(at: old, to: target)
  giveUp("cannot put the new version in place: \(error.localizedDescription)")
}
open(target)
try? files.removeItem(at: old)
try? files.removeItem(at: staged)
try? files.removeItem(at: folder)
exit(0)
