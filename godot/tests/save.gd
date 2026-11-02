## Saves in the middle of a chapter, loads the save, and compares the next
## events with the events after the save. Usage:
## godot --headless --path godot --script res://tests/save.gd -- <folder> <chapter>
extends SceneTree


func _next_texts(player: DlPlayer, count: int) -> Array:
	var texts := []
	while texts.size() < count:
		var e: Dictionary = player.next_event()
		match e.kind:
			"text":
				texts.append("\n".join(e.lines))
			"choice":
				player.choose(1)
				texts.append("choice")
			"select_character":
				player.choose_character(1)
			"monologue":
				for pick in e.picks:
					player.choose_keyword(pick)
				texts.append("monologue")
			"end", "error":
				texts.append(e.kind)
				break
	return texts


func _init() -> void:
	var args := OS.get_cmdline_user_args()
	var player := DlPlayer.new()
	if player.open(args[0]) != "":
		quit(1)
		return
	player.start(args[1])
	_next_texts(player, 40)
	var path := OS.get_user_data_dir().path_join("test-save.json")
	var error: String = player.save_to(path)
	print("save: ", error if error != "" else "ok")
	var after := _next_texts(player, 30)
	var rebuild: Array = player.load_from(path)
	print("rebuild events: ", rebuild.size(), " ", rebuild.map(func(e): return e.kind))
	# The load shows the message of the save again, then the same messages.
	var again := _next_texts(player, 31)
	print("info: ", player.save_info(path))
	var same := again.slice(1) == after
	print("same messages after the load: ", same)
	DirAccess.remove_absolute(path)
	quit(0 if same else 1)
