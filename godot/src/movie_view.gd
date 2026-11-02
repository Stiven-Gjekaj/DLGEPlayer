## Plays a movie over the screen. The Rust library decodes the pictures and
## the sound. A click ends the movie.
class_name MovieView
extends Control

var _skipped := false


func _gui_input(event: InputEvent) -> void:
	if event is InputEventMouseButton and event.pressed:
		_skipped = true
		accept_event()


func _unhandled_key_input(event: InputEvent) -> void:
	if event.is_action_pressed("ui_accept") or event.is_action_pressed("ui_cancel"):
		_skipped = true


func play(path: String) -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	var bg := ColorRect.new()
	bg.color = Color.BLACK
	bg.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	bg.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(bg)
	var view := TextureRect.new()
	view.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	view.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	view.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	view.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(view)

	var movie := DlMovie.new()
	var error: String = movie.open(path)
	if error != "":
		push_warning("movie %s: %s" % [path, error])
		return
	# The sound of a long movie takes about a second to decode. The black
	# screen shows first. The extension of Godot does not accept calls from
	# other threads, so the decode runs here.
	await get_tree().process_frame
	var sound: AudioStreamWAV = DlMovie.sound(path)
	var player := AudioStreamPlayer.new()
	player.bus = "Music"
	add_child(player)
	if sound:
		player.stream = sound
		player.play()

	var texture: ImageTexture = null
	var time := 0.0
	while not movie.finished() and not _skipped:
		await get_tree().process_frame
		time = player.get_playback_position() if player.playing else time + get_process_delta_time()
		if movie.advance(time):
			var image: Image = movie.image()
			if texture == null:
				texture = ImageTexture.create_from_image(image)
				view.texture = texture
			else:
				texture.update(image)
	player.stop()
