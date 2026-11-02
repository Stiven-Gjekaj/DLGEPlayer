## Opens an import folder and plays the first events of a chapter with no
## screen. Usage:
## godot --headless --path godot --script res://tests/smoke.gd -- <folder> <chapter>
extends SceneTree


func _init() -> void:
	var args := OS.get_cmdline_user_args()
	var player := DlPlayer.new()
	var error: String = player.open(args[0])
	if error != "":
		printerr(error)
		quit(1)
		return
	print("chapters: ", player.chapters(false).size())
	print("gallery: ", player.gallery().size())
	error = player.start(args[1])
	if error != "":
		printerr(error)
		quit(1)
		return
	var kinds := {}
	for i in 100000:
		var event: Dictionary = player.next_event()
		var kind: String = event.kind
		kinds[kind] = kinds.get(kind, 0) + 1
		if i < 12:
			print(event)
		if kind == "choice":
			player.choose(0)
		elif kind == "select_character":
			player.choose_character(1)
		elif kind == "end" or kind == "error":
			print(event)
			break
	print(kinds)
	quit(0)
