/// [path] as it is safe to paste into a shell: as it is when nothing in it makes a shell
/// run or send anything, else quoted (single quotes on Linux and macOS, double ones on
/// Windows, where no name holds one).
String pastablePath(String path, {bool windows = false}) {
  final risky = windows
      ? RegExp(r'[&|<>^%;$`!()]')
      : RegExp(r'''[;&|`$<>!'"\\*?\[\]{}~#\n\r\t]''');
  if (!risky.hasMatch(path)) return path;
  return windows ? '"$path"' : "'${path.replaceAll("'", r"'\''")}'";
}
