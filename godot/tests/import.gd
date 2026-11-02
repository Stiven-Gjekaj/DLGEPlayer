## Runs the import of the first start with no screen. Usage:
## godot --headless --path godot --script res://tests/import.gd -- <nsp> <keys> <out> [jobs]
extends SceneTree


func _init() -> void:
	var args := OS.get_cmdline_user_args()
	var importer := DlImporter.new()
	var patch := FileAccess.get_file_as_bytes("res://patch/en.dlpatch")
	print("patch bytes: ", patch.size())
	var jobs := int(args[3]) if args.size() > 3 else 0
	importer.start(args[0], args[1], args[2], patch, false, jobs)
	var shown := 0
	while not importer.done():
		OS.delay_msec(500)
		var lines: PackedStringArray = importer.lines(shown)
		shown += lines.size()
		for line in lines:
			print(line)
	for line in importer.lines(shown):
		print(line)
	var error: String = importer.error()
	print("error: ", error if error != "" else "none")
	var player := DlPlayer.new()
	print("open: ", player.open(args[2]))
	quit(0 if error == "" else 1)
