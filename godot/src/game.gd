## The screen that plays a chapter. It asks the player of the Rust library
## for events and shows them: the background, the sprites, the message
## window, the choices, the audio, and the effects.
class_name GameScreen
extends Control

## The chapter ended. `next` is the next chapter of the route, or "".
signal finished(next: String)
## The player asked for a menu: "save", "load", "backlog", "settings",
## or "title".
signal menu_requested(kind: String)

const FRAME := 1.0 / 60.0
## The touch gestures, in milliseconds and in pixels of the 1920 by 1080
## screen.
const LONG_PRESS_MS := 500
const SWIPE := 160.0
const TOUCH_SLOP := 40.0
## The font size of the message text. A line that is too wide for the box
## makes the size of its message smaller.
const TEXT_SIZE := 48
const ADV := "ui/%s_advUI/UI_06_1^q.png"

## The x position of the centre of a sprite for each slot of `0x480`.
## The scripts use 1 or 3 for one sprite in the middle, 6 and 2 for a
## pair on the left and the right, and 4 and 0 for another pair. These
## values are a guess from the scripts.
const SLOT_X := {0: 1380.0, 1: 960.0, 2: 1300.0, 3: 960.0, 4: 560.0, 5: 960.0, 6: 620.0}
## The pixels for one unit of the offset of `0x480`.
const OFFSET_UNIT := 24.0
## The colours of `0x488` and `0x48a`. They are a guess.
const TINTS := {
	0: Color.BLACK, 1: Color.WHITE, 2: Color.BLACK, 4: Color(0.35, 0.25, 0.15),
	5: Color.WHITE, 7: Color(0.45, 0.0, 0.03),
}

var stage := Control.new()
var black := ColorRect.new()
var bg_back := TextureRect.new()
var bg_front := TextureRect.new()
var figures := Control.new()
var fg := TextureRect.new()
var tint := ColorRect.new()
var snapshot := TextureRect.new()
var window := Control.new()
var face_holder := Control.new()
var plate := TextureRect.new()
var name_label := Label.new()
var text_label := RichTextLabel.new()
var next_mark := Label.new()
var choices := VBoxContainer.new()
var spot_layer := Control.new()
var bar := HBoxContainer.new()
var notice := Label.new()

var music_players: Array[AudioStreamPlayer] = []
var music_index := 0
var ambience := AudioStreamPlayer.new()
var sound := AudioStreamPlayer.new()
var voice := AudioStreamPlayer.new()
var extra_voice := AudioStreamPlayer.new()

var figure_nodes := {}
var running := false
var auto := false
var skip := false
var window_wanted := true
var user_hidden := false
var transition_pending := false
var transition_frames := 0
var _clicks := 0
var _pick := -1
var _skip_held := false
var _touch_start := Vector2.ZERO
var _touch_time := 0


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	clip_contents = true
	_build()


func _full(c: Control) -> void:
	c.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	c.mouse_filter = Control.MOUSE_FILTER_IGNORE


func _build() -> void:
	_full(stage)
	stage.pivot_offset = Vector2(960, 540)
	add_child(stage)
	black.color = Color.BLACK
	_full(black)
	stage.add_child(black)
	for bg in [bg_back, bg_front]:
		_full(bg)
		bg.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
		bg.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_COVERED
		stage.add_child(bg)
	_full(figures)
	stage.add_child(figures)
	_full(fg)
	fg.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	fg.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_COVERED
	stage.add_child(fg)
	_full(tint)
	tint.color = Color(0, 0, 0, 0)
	add_child(tint)
	_full(snapshot)
	snapshot.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	snapshot.stretch_mode = TextureRect.STRETCH_SCALE
	add_child(snapshot)

	_full(window)
	add_child(window)
	var adv := ADV % App.game
	var back := TextureRect.new()
	back.texture = Ui.region(adv, Rect2(0, 0, 1920, 288))
	back.position = Vector2(0, 792)
	back.mouse_filter = Control.MOUSE_FILTER_IGNORE
	window.add_child(back)
	var deco := TextureRect.new()
	deco.texture = Ui.region(adv, Rect2(896, 292, 269, 364))
	deco.position = Vector2(0, 716)
	deco.mouse_filter = Control.MOUSE_FILTER_IGNORE
	window.add_child(deco)
	face_holder.position = Vector2(14, 806)
	face_holder.size = Vector2(270, 270)
	face_holder.clip_contents = true
	face_holder.mouse_filter = Control.MOUSE_FILTER_IGNORE
	window.add_child(face_holder)
	plate.texture = Ui.region(adv, Rect2(1160, 288, 696, 128))
	plate.position = Vector2(800, 700)
	plate.mouse_filter = Control.MOUSE_FILTER_IGNORE
	window.add_child(plate)
	name_label.position = Vector2(55, 28)
	name_label.size = Vector2(601, 65)
	name_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	name_label.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	name_label.add_theme_font_size_override("font_size", 46)
	name_label.add_theme_color_override("font_color", Color(0.04, 0, 0))
	plate.add_child(name_label)
	text_label.position = Vector2(561, 840)
	text_label.size = Vector2(1300, 230)
	text_label.bbcode_enabled = false
	text_label.scroll_active = false
	text_label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	text_label.mouse_filter = Control.MOUSE_FILTER_IGNORE
	text_label.add_theme_font_size_override("normal_font_size", TEXT_SIZE)
	text_label.add_theme_constant_override("line_separation", 12)
	text_label.add_theme_constant_override("outline_size", 8)
	text_label.add_theme_color_override("font_outline_color", Color(0, 0, 0, 0.8))
	window.add_child(text_label)
	next_mark.text = "▼"
	next_mark.position = Vector2(1840, 1020)
	next_mark.add_theme_font_size_override("font_size", 32)
	next_mark.add_theme_color_override("font_color", Ui.GOLD)
	window.add_child(next_mark)

	choices.position = Vector2(410, 260)
	choices.size = Vector2(1100, 400)
	choices.alignment = BoxContainer.ALIGNMENT_CENTER
	choices.add_theme_constant_override("separation", 28)
	add_child(choices)
	_full(spot_layer)
	add_child(spot_layer)

	bar.position = Vector2(760, 8)
	bar.size = Vector2(1152, 56)
	bar.alignment = BoxContainer.ALIGNMENT_END
	bar.add_theme_constant_override("separation", 6)
	for item in [
		["Hide", _toggle_window], ["Auto", _toggle_auto], ["Skip", _toggle_skip], ["Log", menu_requested.emit.bind("backlog")],
		["Save", menu_requested.emit.bind("save")], ["Load", menu_requested.emit.bind("load")],
		["Config", menu_requested.emit.bind("settings")], ["Title", menu_requested.emit.bind("title")],
	]:
		var b := Ui.button(item[0], item[1], 26)
		b.focus_mode = Control.FOCUS_NONE
		bar.add_child(b)
	add_child(bar)
	notice.position = Vector2(40, 20)
	notice.add_theme_font_size_override("font_size", 30)
	notice.add_theme_color_override("font_color", Ui.GOLD)
	add_child(notice)

	for i in 2:
		var p := AudioStreamPlayer.new()
		p.bus = "Music"
		add_child(p)
		music_players.append(p)
	ambience.bus = "Sound"
	sound.bus = "Sound"
	voice.bus = "Voice"
	extra_voice.bus = "Voice"
	for p in [ambience, sound, voice, extra_voice]:
		add_child(p)
	_set_window(false)


## Plays events from the player until the chapter ends.
func play(rebuild: Array = []) -> void:
	running = true
	for e in rebuild:
		await _handle(e)
	while running:
		var e: Dictionary = App.player.next_event()
		await _handle(e)


func stop() -> void:
	running = false
	voice.stop()
	extra_voice.stop()
	for p in music_players:
		p.stop()
	ambience.stop()
	sound.stop()


func _skipping() -> bool:
	return skip or _skip_held


func _seconds(frames: int) -> float:
	if _skipping() or transition_pending:
		return 0.0
	return maxf(frames, 0) * FRAME


func _handle(e: Dictionary) -> void:
	match e.kind:
		"background":
			_set_background(e.file, _seconds(e.frames))
		"foreground":
			_fade_texture(fg, App.texture(e.file) if e.file else null, _seconds(e.frames))
		"figure":
			_show_figure(e)
		"look":
			var f: Figure = figure_nodes.get(e.key)
			if f:
				f.set_look(e)
		"move":
			var f: Figure = figure_nodes.get(e.key)
			if f:
				var t := create_tween()
				t.tween_property(f, "position:x", _figure_x(e.slot, 0, f.size.x), _seconds(e.frames))
		"hide":
			_hide_figure(e.key, _seconds(e.frames))
		"music":
			_play_music(e)
		"music_stop":
			_stop_music(e.fade / 1000.0)
		"ambience":
			var s := App.audio(e.file)
			if s:
				s.loop = true
				ambience.stream = s
				ambience.play()
		"ambience_stop":
			ambience.stop()
		"sound":
			if not _skipping():
				sound.stream = App.audio(e.file)
				sound.play()
		"sound_wait":
			await _wait_sound()
		"extra_voice":
			if not _skipping():
				extra_voice.stream = App.audio(e.file)
				extra_voice.play()
		"text":
			await _show_text(e)
		"choice":
			await _choice(e.options)
		"monologue":
			await _monologue(e.options, e.picks)
		"select_character":
			await _select_character()
		"wait":
			_flush_transition()
			await _wait_time(_seconds(e.frames))
		"window":
			window_wanted = e.visible
			_set_window(e.visible)
		"transition":
			await _begin_transition(e.frames)
		"tint":
			var c: Color = TINTS.get(int(e.colour), Color.BLACK)
			c.a = e.alpha / 255.0
			create_tween().tween_property(tint, "color", c, _seconds(e.frames))
		"tint_end":
			var c := tint.color
			c.a = 0.0
			create_tween().tween_property(tint, "color", c, _seconds(e.frames))
		"shake":
			_shake(e)
		"camera":
			_camera(e)
		"movie":
			_flush_transition()
			await _movie(e.file)
		"hotspots":
			_flush_transition()
			await _hotspots(e.spots)
		"end":
			_flush_transition()
			running = false
			App.save_profile()
			finished.emit(e.next if e.next else "")
		"error":
			push_error(e.message)
			running = false
			notice.text = "Error: " + e.message
			await get_tree().create_timer(3.0).timeout
			finished.emit("")
		_:
			pass


# The background, the sprites, and the effects.

func _fade_texture(rect: TextureRect, texture: Texture2D, seconds: float) -> void:
	if seconds <= 0.0:
		rect.texture = texture
		rect.modulate.a = 1.0
		return
	if texture == null:
		var t := create_tween()
		t.tween_property(rect, "modulate:a", 0.0, seconds)
		t.tween_callback(func(): rect.texture = null; rect.modulate.a = 1.0)
		return
	rect.texture = texture
	rect.modulate.a = 0.0
	create_tween().tween_property(rect, "modulate:a", 1.0, seconds)


func _set_background(file: Variant, seconds: float) -> void:
	var texture: Texture2D = App.texture(file) if file else null
	bg_back.texture = bg_front.texture
	bg_back.modulate.a = 1.0
	_fade_texture(bg_front, texture, seconds)
	if texture == null:
		bg_back.texture = null


func _figure_x(slot: int, offset: int, width: float) -> float:
	return SLOT_X.get(slot, 960.0) + offset * OFFSET_UNIT - width / 2.0


func _is_face(e: Dictionary) -> bool:
	return int(e.slot) == -2 or String(e.key).begins_with("her")


func _show_figure(e: Dictionary) -> void:
	var f: Figure = figure_nodes.get(e.key)
	var face := _is_face(e)
	if f == null:
		f = Figure.new()
		figure_nodes[e.key] = f
		(face_holder if face else figures).add_child(f)
		f.modulate.a = 0.0
	f.set_look(e)
	if face:
		f.scale = Vector2(270.0, 270.0) / f.size
		f.position = Vector2.ZERO
	else:
		f.position = Vector2(_figure_x(e.slot, e.offset, f.size.x), 1080.0 - f.size.y)
		f.set_meta("layer", int(e.layer))
		_sort_figures()
	var seconds := _seconds(e.frames)
	if seconds <= 0.0:
		f.modulate.a = 1.0
	else:
		create_tween().tween_property(f, "modulate:a", 1.0, seconds)


## Puts the sprites with a higher layer number behind the others.
func _sort_figures() -> void:
	var list := figures.get_children()
	list.sort_custom(func(a, b): return int(a.get_meta("layer", 0)) > int(b.get_meta("layer", 0)))
	for i in list.size():
		figures.move_child(list[i], i)


func _hide_figure(key: String, seconds: float) -> void:
	var f: Figure = figure_nodes.get(key)
	if f == null:
		return
	figure_nodes.erase(key)
	if seconds <= 0.0:
		f.queue_free()
		return
	var t := create_tween()
	t.tween_property(f, "modulate:a", 0.0, seconds)
	t.tween_callback(f.queue_free)


func _shake(e: Dictionary) -> void:
	if _skipping() or int(e.size) == 0:
		return
	var seconds := maxf(e.frames, 1) * FRAME
	var axis := Vector2.RIGHT if int(e.axis) == 0 else Vector2.DOWN
	var t := create_tween()
	var steps := maxi(int(e.count), 1) * 2
	for i in steps:
		var sign := 1.0 if i % 2 == 0 else -1.0
		t.tween_property(stage, "position", axis * sign * e.size, seconds / steps)
	t.tween_property(stage, "position", Vector2.ZERO, seconds / steps)


func _camera(e: Dictionary) -> void:
	var zoom: float = e.zoom if e.zoom > 0.0 else 1.0
	var t := create_tween().set_parallel()
	var seconds := _seconds(e.frames)
	t.tween_property(stage, "scale", Vector2(zoom, zoom), seconds)
	t.tween_property(stage, "position", Vector2(-e.x, -e.y) * (zoom - 1.0), seconds)


func _begin_transition(frames: int) -> void:
	if _skipping() or frames <= 0:
		return
	_flush_transition()
	await RenderingServer.frame_post_draw
	var image := get_viewport().get_texture().get_image()
	snapshot.texture = ImageTexture.create_from_image(image)
	snapshot.modulate.a = 1.0
	snapshot.visible = true
	transition_pending = true
	transition_frames = frames


## Fades out the picture of the screen from before a transition.
func _flush_transition() -> void:
	if not transition_pending:
		return
	transition_pending = false
	var t := create_tween()
	t.tween_property(snapshot, "modulate:a", 0.0, transition_frames * FRAME)
	t.tween_callback(func(): snapshot.visible = false; snapshot.texture = null)


# The audio.

func _play_music(e: Dictionary) -> void:
	var s := App.audio(e.file)
	if s == null:
		return
	s.loop = true
	s.loop_offset = float(e.loop_start) / maxf(float(e.sample_rate), 1.0)
	var old := music_players[music_index]
	music_index = 1 - music_index
	var p := music_players[music_index]
	p.stream = s
	var fade: float = e.fade / 1000.0
	p.volume_db = -60.0 if fade > 0.0 else 0.0
	p.play()
	var t := create_tween().set_parallel()
	if fade > 0.0:
		t.tween_property(p, "volume_db", 0.0, fade)
	t.tween_property(old, "volume_db", -60.0, maxf(fade, 0.3))
	t.chain().tween_callback(old.stop)


func _stop_music(seconds: float) -> void:
	var p := music_players[music_index]
	if not p.playing:
		return
	if seconds <= 0.0:
		p.stop()
		return
	var t := create_tween()
	t.tween_property(p, "volume_db", -60.0, seconds)
	t.tween_callback(p.stop)


func _wait_sound() -> void:
	var waited := 0.0
	while sound.playing and waited < 6.0 and not _skipping():
		var before := _clicks
		await get_tree().process_frame
		waited += get_process_delta_time()
		if _clicks != before:
			return


# The message window and the input.

func _set_window(visible_now: bool) -> void:
	window.visible = visible_now and not user_hidden
	bar.visible = not user_hidden


func _tap() -> void:
	if user_hidden:
		user_hidden = false
		_set_window(window_wanted)
	else:
		_clicks += 1


func _toggle_window() -> void:
	user_hidden = not user_hidden
	_set_window(window_wanted)


## Touch: a tap is a left click, a long press hides the message window,
## and a swipe down opens the backlog. Godot also makes a mouse event from
## each touch. The screen ignores that event.
func _touch(event: InputEventScreenTouch) -> void:
	if event.index != 0:
		return
	if event.pressed:
		_touch_start = event.position
		_touch_time = Time.get_ticks_msec()
		return
	var moved := event.position - _touch_start
	if moved.y > SWIPE and absf(moved.x) < moved.y:
		menu_requested.emit("backlog")
	elif moved.length() < TOUCH_SLOP:
		if Time.get_ticks_msec() - _touch_time >= LONG_PRESS_MS:
			_toggle_window()
		else:
			_tap()


func _gui_input(event: InputEvent) -> void:
	if event is InputEventScreenTouch:
		_touch(event)
		accept_event()
		return
	if event is InputEventMouseButton and event.pressed:
		if event.device == InputEvent.DEVICE_ID_EMULATION:
			accept_event()
			return
		match event.button_index:
			MOUSE_BUTTON_LEFT:
				_tap()
				accept_event()
			MOUSE_BUTTON_RIGHT:
				_toggle_window()
				accept_event()
			MOUSE_BUTTON_WHEEL_UP:
				menu_requested.emit("backlog")
				accept_event()


func _unhandled_key_input(event: InputEvent) -> void:
	if not is_visible_in_tree():
		return
	if event.is_action_pressed("ui_accept"):
		_clicks += 1
	elif event is InputEventKey and event.keycode == KEY_CTRL:
		_skip_held = event.pressed
	elif event.is_action_pressed("ui_cancel"):
		menu_requested.emit("settings")


func _toggle_auto() -> void:
	auto = not auto
	skip = false
	_update_notice()


func _toggle_skip() -> void:
	skip = not skip
	auto = false
	_update_notice()


func _update_notice() -> void:
	notice.text = "AUTO" if auto else ("SKIP" if skip else "")


## Waits for a click. A skip ends the wait at once. Auto mode ends it
## after `auto_seconds`.
func _wait_click(auto_seconds: float = -1.0) -> void:
	var before := _clicks
	var waited := 0.0
	while _clicks == before:
		if _skipping() or not running:
			await get_tree().process_frame
			return
		await get_tree().process_frame
		waited += get_process_delta_time()
		if auto and auto_seconds >= 0.0 and waited >= auto_seconds and not voice.playing:
			return


func _wait_time(seconds: float) -> void:
	var waited := 0.0
	var before := _clicks
	while waited < seconds and _clicks == before and not _skipping():
		await get_tree().process_frame
		waited += get_process_delta_time()


func _speaking_figure(character: String) -> Figure:
	if character == "":
		return null
	for key in figure_nodes:
		if String(key).begins_with(character):
			return figure_nodes[key]
	return null


## Gives the largest font size, up to TEXT_SIZE, that shows each line of a
## message on one line. The English lines have breaks for the font of the
## game, and some of them are too wide for the font of the player.
func _text_size(lines: Array) -> int:
	var font := text_label.get_theme_font("normal_font")
	var widest := 0.0
	for line in lines:
		widest = maxf(widest, font.get_string_size(line, HORIZONTAL_ALIGNMENT_LEFT, -1, TEXT_SIZE).x)
	var room := text_label.size.x - 4.0
	if widest <= room:
		return TEXT_SIZE
	return int(TEXT_SIZE * room / widest)


func _show_text(e: Dictionary) -> void:
	_flush_transition()
	if skip and not e.read and not App.setting("skip_unread", false):
		skip = false
		_update_notice()
	var name: String = e.speaker
	plate.visible = name != ""
	name_label.text = name
	text_label.text = "\n".join(e.lines)
	text_label.add_theme_font_size_override("normal_font_size", _text_size(e.lines))
	next_mark.visible = false
	_set_window(true)
	if e.voice and not _skipping():
		voice.stream = App.audio(e.voice)
		voice.play()
		var f := _speaking_figure(e.character)
		if f:
			f.speak(voice, e.lip)
	elif App.setting("voice_stops", true):
		voice.stop()
	var speed: float = App.setting("text_speed", 45.0)
	var total := text_label.get_total_character_count()
	if _skipping() or speed >= 200.0:
		text_label.visible_characters = -1
	else:
		text_label.visible_characters = 0
		var before := _clicks
		var shown := 0.0
		while shown < total and _clicks == before and not _skipping():
			await get_tree().process_frame
			shown += speed * get_process_delta_time()
			text_label.visible_characters = int(shown)
		text_label.visible_characters = -1
	next_mark.visible = true
	var auto_delay: float = App.setting("auto_delay", 1.5)
	await _wait_click(auto_delay + total * 0.02)
	if App.setting("voice_stops", true):
		voice.stop()


func _choice(options: Array) -> void:
	skip = false
	_skip_held = false
	_update_notice()
	_pick = -1
	for c in choices.get_children():
		c.queue_free()
	for i in options.size():
		var b := Ui.button(options[i], func(): _pick = i, 44)
		b.custom_minimum_size = Vector2(1100, 96)
		choices.add_child(b)
	choices.visible = true
	while _pick < 0 and running:
		await get_tree().process_frame
	choices.visible = false
	for c in choices.get_children():
		c.queue_free()
	App.player.choose(_pick)


## Shows the keyword pick at the end of a monologue. Each option is the
## line of one keyword. The player picks `picks` of them, and each pick
## shows if the keyword is correct.
func _monologue(options: Array, picks: int) -> void:
	skip = false
	_skip_held = false
	_update_notice()
	for c in choices.get_children():
		c.queue_free()
	var place := [choices.position, choices.size]
	choices.position = Vector2(160, 200)
	choices.size = Vector2(1600, 560)
	var title := Ui.label("Pick %d keywords" % picks, 40)
	title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	choices.add_child(title)
	var buttons: Array[Button] = []
	for i in options.size():
		var b := Ui.button(options[i], func(): _pick = i, 32)
		b.custom_minimum_size = Vector2(1600, 88)
		b.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		choices.add_child(b)
		buttons.append(b)
	choices.visible = true
	var left := picks
	while left > 0 and running:
		_pick = -1
		while _pick < 0 and running:
			await get_tree().process_frame
		if _pick < 0:
			break
		var correct: int = App.player.choose_keyword(_pick)
		if correct < 0:
			continue
		buttons[_pick].disabled = true
		buttons[_pick].text = ("True: " if correct == 1 else "False: ") + options[_pick]
		left -= 1
	if running:
		await get_tree().create_timer(1.0).timeout
	choices.visible = false
	for c in choices.get_children():
		c.queue_free()
	choices.position = place[0]
	choices.size = place[1]


## The six brothers in the order of the value of the flag `character`. The
## order is a guess.
const CHARACTERS := ["Shu", "Reiji", "Ayato", "Kanato", "Laito", "Subaru"]


func _select_character() -> void:
	await _choice_titles(CHARACTERS)
	App.player.choose_character(_pick + 1)


func _choice_titles(titles: Array) -> void:
	_pick = -1
	for c in choices.get_children():
		c.queue_free()
	for i in titles.size():
		var b := Ui.button(titles[i], func(): _pick = i, 44)
		b.custom_minimum_size = Vector2(800, 80)
		choices.add_child(b)
	choices.visible = true
	while _pick < 0 and running:
		await get_tree().process_frame
	choices.visible = false
	for c in choices.get_children():
		c.queue_free()


func _hotspots(spots: Array) -> void:
	var left := spots.size()
	for s in spots:
		var b := Button.new()
		b.text = "♥"
		b.flat = true
		b.add_theme_font_size_override("font_size", 64)
		b.add_theme_color_override("font_color", Color(1, 0.4, 0.5))
		b.position = Vector2(s.x * 2.0 - 40.0, s.y * 2.0 - 40.0)
		b.size = Vector2(80, 80)
		var path: Variant = s.voice
		b.pressed.connect(func():
			if path:
				voice.stream = App.audio(path)
				voice.play()
			b.queue_free()
			left -= 1)
		spot_layer.add_child(b)
	while left > 0 and running and not _skipping():
		await get_tree().process_frame
	for c in spot_layer.get_children():
		c.queue_free()


func _movie(file: String) -> void:
	var view := MovieView.new()
	add_child(view)
	await view.play(App.path(file))
	view.queue_free()
