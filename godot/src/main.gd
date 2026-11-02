## The root of the program. It shows one screen at a time and opens the
## menus over the game.
extends Control

const GAMES := {"ve": "Haunted Dark Bridal", "mb": "MORE,BLOOD"}

var current: Control
var game: GameScreen
var overlay: Control


func _ready() -> void:
	theme = Ui.theme()
	# The back button of Android closes a menu. It does not quit the game.
	get_tree().quit_on_go_back = false
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_make_buses()
	App.apply_audio()
	var args := _arguments()
	if App.open_import(args.get("folder", "")) != "":
		show_setup()
	elif args.has("screen"):
		App.game = args.get("game", "ve")
		call("show_" + args.screen)
	elif args.has("movie"):
		show_title()
		play_movie(args.movie)
	elif args.has("chapter"):
		App.game = String(args.chapter).left(2)
		start_chapter(args.chapter)
	else:
		show_title()
	if args.has("click"):
		_auto_click(float(args.click))


func _notification(what: int) -> void:
	if what == NOTIFICATION_WM_GO_BACK_REQUEST:
		_go_back()


## The back button of Android: it closes a menu, opens the settings in a
## chapter, or goes to the title.
func _go_back() -> void:
	if overlay:
		close_overlay()
	elif game:
		_on_menu("settings")
	elif current and App.root != "":
		show_title()


## Reads the options after `--` on the command line: `--folder <dir>`,
## `--chapter <name>`, `--movie <name>`, `--screen <name>` with
## `--game <ve or mb>`, and `--click <seconds>`. A test uses them with the
## movie writer of Godot to save pictures of a chapter.
func _arguments() -> Dictionary:
	var out := {}
	var args := OS.get_cmdline_user_args()
	for i in args.size() - 1:
		if args[i].begins_with("--"):
			out[args[i].substr(2)] = args[i + 1]
	return out


## Clicks the game screen at a fixed interval.
func _auto_click(every: float) -> void:
	while true:
		await get_tree().create_timer(every).timeout
		if game:
			game._clicks += 1


func _make_buses() -> void:
	for bus in ["Music", "Voice", "Sound"]:
		if AudioServer.get_bus_index(bus) < 0:
			AudioServer.add_bus()
			var index := AudioServer.bus_count - 1
			AudioServer.set_bus_name(index, bus)
			AudioServer.set_bus_send(index, "Master")


func _switch(screen: Control) -> void:
	close_overlay()
	if game:
		game.stop()
		game = null
	if current:
		current.queue_free()
	current = screen
	add_child(screen)


func close_overlay() -> void:
	if overlay:
		overlay.queue_free()
		overlay = null


func _open_overlay(screen: Control) -> void:
	close_overlay()
	overlay = screen
	add_child(screen)


func _column(parent: Control, at: Vector2, width: float) -> VBoxContainer:
	var box := VBoxContainer.new()
	box.position = at
	box.size = Vector2(width, 0)
	box.add_theme_constant_override("separation", 18)
	parent.add_child(box)
	return box


# The first start: the player gives the NSP file and the keys, or a
# folder that an earlier import made.

const PATCH := "res://patch/en.dlpatch"

var _setup := {"input": "", "keys": "", "out": ""}


func show_setup(message: String = "") -> void:
	if _setup.out == "":
		_setup.out = OS.get_system_dir(OS.SYSTEM_DIR_DOCUMENTS).path_join("DLGE Player")
	var s := Ui.screen(1.0)
	var box := _column(s, Vector2(200, 80), 1520)
	box.add_child(Ui.label("DLGE Player", 72, Ui.GOLD))
	var text := Ui.label(
		"The player reads the game from your own copy. Give the NSP file of Diabolik Lovers "
		+ "Grand Edition and the prod.keys file of your console. The import converts the game "
		+ "one time into a folder. After the import, you can delete the NSP file and prod.keys.", 30)
	text.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	box.add_child(text)
	_file_row(box, "Game (NSP)", "input", ["*.nsp"], FileDialog.FILE_MODE_OPEN_FILE)
	_file_row(box, "prod.keys", "keys", ["*.keys"], FileDialog.FILE_MODE_OPEN_FILE)
	_file_row(box, "Import folder", "out", [], FileDialog.FILE_MODE_OPEN_DIR)
	var english := CheckBox.new()
	english.text = "English translation"
	english.button_pressed = FileAccess.file_exists(PATCH)
	english.disabled = not FileAccess.file_exists(PATCH)
	english.add_theme_font_size_override("font_size", 32)
	box.add_child(english)
	var small := CheckBox.new()
	small.text = "Smaller files (WebP images, lower audio quality)"
	small.add_theme_font_size_override("font_size", 32)
	box.add_child(small)
	if message != "":
		var m := Ui.label(message, 30, Color(1, 0.55, 0.55))
		m.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		box.add_child(m)
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 20)
	row.add_child(Ui.button("Start the import", func():
		if _setup.input == "" or _setup.keys == "":
			show_setup("Select the NSP file and prod.keys first.")
		else:
			_run_import(english.button_pressed, small.button_pressed)))
	row.add_child(Ui.button("Open an import folder", func():
		_pick(FileDialog.FILE_MODE_OPEN_DIR, [], func(dir: String):
			var error := App.open_import(dir)
			if error == "":
				show_title()
			else:
				show_setup(error))))
	# A phone app does not quit itself.
	if not App.mobile:
		row.add_child(Ui.button("Quit", get_tree().quit))
	box.add_child(row)
	_switch(s)


func _file_row(box: VBoxContainer, title: String, key: String, filters: Array, mode: FileDialog.FileMode) -> void:
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 20)
	var name := Ui.label(title, 32, Ui.GOLD)
	name.custom_minimum_size = Vector2(280, 0)
	row.add_child(name)
	row.add_child(Ui.button("Select", func():
		_pick(mode, filters, func(path: String):
			_setup[key] = path
			show_setup()), 28))
	var value := Ui.label(_setup[key] if _setup[key] != "" else "(none)", 26)
	value.clip_text = true
	value.custom_minimum_size = Vector2(1000, 0)
	row.add_child(value)
	box.add_child(row)


func _pick(mode: FileDialog.FileMode, filters: Array, done: Callable) -> void:
	var dialog := FileDialog.new()
	dialog.file_mode = mode
	dialog.access = FileDialog.ACCESS_FILESYSTEM
	dialog.use_native_dialog = true
	dialog.filters = PackedStringArray(filters)
	dialog.file_selected.connect(done)
	dialog.dir_selected.connect(done)
	add_child(dialog)
	dialog.popup_centered(Vector2i(1200, 800))


func _run_import(english: bool, small: bool) -> void:
	var s := Ui.screen(1.0)
	var box := _column(s, Vector2(200, 80), 1520)
	box.add_child(Ui.label("Importing", 64, Ui.GOLD))
	var status := Ui.label("This takes some minutes. The player uses half of the CPU cores.", 30)
	box.add_child(status)
	var log := Ui.label("", 26)
	log.custom_minimum_size = Vector2(1500, 700)
	log.vertical_alignment = VERTICAL_ALIGNMENT_BOTTOM
	log.clip_text = true
	box.add_child(log)
	_switch(s)
	var importer := DlImporter.new()
	var patch := FileAccess.get_file_as_bytes(PATCH) if english else PackedByteArray()
	importer.start(_setup.input, _setup.keys, _setup.out, patch, small, 0)
	var lines: Array[String] = []
	var start := Time.get_ticks_msec()
	while not importer.done():
		await get_tree().create_timer(0.5).timeout
		lines.append_array(importer.lines(lines.size()))
		log.text = "\n".join(lines.slice(maxi(lines.size() - 22, 0)))
		status.text = "%d s" % ((Time.get_ticks_msec() - start) / 1000)
	lines.append_array(importer.lines(lines.size()))
	var error: String = importer.error()
	if error != "":
		show_setup("The import failed: " + error)
		return
	error = App.open_import(_setup.out)
	if error != "":
		show_setup(error)
		return
	status.text = ("The import is complete. You can delete the NSP file and prod.keys now. "
		+ "The player does not need them again.")
	status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	status.custom_minimum_size = Vector2(1500, 0)
	box.add_child(Ui.button("Continue", show_title))


# The title screen.

func show_title() -> void:
	var s := Ui.screen(1.0)
	var art := TextureRect.new()
	art.texture = App.texture("images/title_char/%d.png" % (1 if App.game == "ve" else 2))
	art.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	art.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	art.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_COVERED
	art.modulate = Color(1, 1, 1, 0.55)
	s.add_child(art)
	var box := _column(s, Vector2(120, 120), 700)
	box.add_child(Ui.label("Diabolik Lovers\nGrand Edition", 64, Ui.GOLD))
	var games := HBoxContainer.new()
	games.add_theme_constant_override("separation", 12)
	for key in GAMES:
		var b := Ui.button(GAMES[key], func(): App.game = key; show_title(), 34)
		b.disabled = App.game == key
		games.add_child(b)
	box.add_child(games)
	box.add_child(Ui.label(GAMES[App.game], 44))
	box.add_child(Ui.button("Chapters", show_chapters))
	var latest := _latest_save()
	var cont := Ui.button("Continue", func(): load_slot(latest))
	cont.disabled = latest < 0
	box.add_child(cont)
	box.add_child(Ui.button("Load", func(): _open_overlay(_saves(false))))
	box.add_child(Ui.button("Opening", func(): play_movie(App.game + "_op")))
	box.add_child(Ui.button("Gallery", show_gallery))
	box.add_child(Ui.button("Music", show_music))
	box.add_child(Ui.button("Settings", func(): _open_overlay(_settings())))
	if not App.mobile:
		box.add_child(Ui.button("Quit", get_tree().quit))
	_switch(s)


func _latest_save() -> int:
	var best := -1
	var best_time := -1
	for slot in App.SAVE_SLOTS:
		var info: Dictionary = App.player.save_info(App.save_path(slot))
		if not info.is_empty() and int(info.time) > best_time:
			best_time = int(info.time)
			best = slot
	return best


## Plays a movie of the import folder by name, for example `ve_op`.
func play_movie(name: String) -> void:
	var view := MovieView.new()
	add_child(view)
	await view.play(App.path("movies/%s.mp4" % name))
	view.queue_free()


# The chapters.

func show_chapters() -> void:
	var s := Ui.screen(1.0)
	s.add_child(_header("Chapters: " + GAMES[App.game], show_title))
	var scroll := ScrollContainer.new()
	scroll.position = Vector2(80, 130)
	scroll.size = Vector2(1760, 920)
	s.add_child(scroll)
	var list := VBoxContainer.new()
	list.add_theme_constant_override("separation", 10)
	scroll.add_child(list)
	var open_all: bool = App.setting("open_all", false)
	var routes := {}
	var order := []
	for c in App.player.chapters(open_all):
		if (int(c.game) == 0) != (App.game == "ve"):
			continue
		var key := "%s %s" % [c.character, c.route]
		if not routes.has(key):
			routes[key] = []
			order.append(key)
		routes[key].append(c)
	order.sort_custom(func(a, b): return _route_rank(routes[a][0]) < _route_rank(routes[b][0]))
	for key in order:
		var row := HBoxContainer.new()
		row.add_theme_constant_override("separation", 8)
		var first: Dictionary = routes[key][0]
		var title := Ui.label(_route_title(first), 32, Ui.GOLD)
		title.custom_minimum_size = Vector2(330, 0)
		row.add_child(title)
		for c in routes[key]:
			var label := "%02d" % int(c.number)
			if c.finished:
				label += " ✓"
			var b := Ui.button(label, start_chapter.bind(c.name), 28)
			b.disabled = not c.open
			b.tooltip_text = c.name
			row.add_child(b)
		list.add_child(row)
	_switch(s)


const NAMES := {
	"shu": "Shu", "rei": "Reiji", "aya": "Ayato", "kan": "Kanato", "lai": "Laito",
	"sub": "Subaru", "ruk": "Ruki", "kou": "Kou", "yum": "Yuma", "azu": "Azusa",
}


const ROUTES := ["prologue", "Dark", "Maniac", "Ecstasy", "ED", "Heaven", "After"]


## Gives a number that puts the routes in order: the prologue first, then
## each character with its routes in the order of the game.
func _route_rank(c: Dictionary) -> int:
	var character := NAMES.keys().find(c.character) + 1
	var route := ROUTES.find(c.route)
	return character * 100 + (route if route >= 0 else 99)


func _route_title(c: Dictionary) -> String:
	if c.route == "prologue":
		return "Prologue"
	return "%s: %s" % [NAMES.get(c.character, c.character), c.route]


func _header(title: String, back: Callable) -> HBoxContainer:
	var row := HBoxContainer.new()
	row.position = Vector2(80, 30)
	row.size = Vector2(1760, 80)
	row.add_theme_constant_override("separation", 24)
	row.add_child(Ui.button("Back", back, 32))
	row.add_child(Ui.label(title, 48, Ui.GOLD))
	return row


# The game.

func start_chapter(chapter: String) -> void:
	var error: String = App.player.start(chapter)
	if error != "":
		push_error(error)
		return
	# The events show the background of the chapter before.
	_play(App.player.rebuild())


func load_slot(slot: int) -> void:
	if slot < 0:
		return
	var events: Array = App.player.load_from(App.save_path(slot))
	if events.size() == 1 and events[0].kind == "error":
		push_error(events[0].message)
		return
	_play(events)


func _play(rebuild: Array) -> void:
	var screen := GameScreen.new()
	_switch(screen)
	game = screen
	screen.finished.connect(_on_chapter_end)
	screen.menu_requested.connect(_on_menu)
	screen.play(rebuild)


func _on_chapter_end(next: String) -> void:
	App.player.stop()
	App.save_profile()
	if next != "":
		start_chapter(next)
	else:
		show_chapters()


func _on_menu(kind: String) -> void:
	match kind:
		"save":
			_open_overlay(_saves(true))
		"load":
			_open_overlay(_saves(false))
		"backlog":
			_open_overlay(_backlog())
		"settings":
			_open_overlay(_settings())
		"title":
			App.player.stop()
			App.save_profile()
			show_title()


# The save and load menu.

func _saves(saving: bool, page: int = 0) -> Control:
	var s := Ui.screen(0.94)
	s.add_child(_header(("Save" if saving else "Load") + "  page %d" % (page + 1), close_overlay))
	var pages := HBoxContainer.new()
	pages.position = Vector2(1100, 34)
	for p in App.SAVE_SLOTS / 12:
		var b := Ui.button(str(p + 1), func(): _open_overlay(_saves(saving, p)), 30)
		b.disabled = p == page
		pages.add_child(b)
	s.add_child(pages)
	var grid := GridContainer.new()
	grid.columns = 3
	grid.position = Vector2(80, 140)
	grid.add_theme_constant_override("h_separation", 20)
	grid.add_theme_constant_override("v_separation", 20)
	s.add_child(grid)
	for i in 12:
		var slot := page * 12 + i
		var info: Dictionary = App.player.save_info(App.save_path(slot))
		var b := Button.new()
		b.custom_minimum_size = Vector2(570, 210)
		b.clip_contents = true
		var thumb := TextureRect.new()
		thumb.position = Vector2(10, 10)
		thumb.size = Vector2(336, 190)
		thumb.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
		thumb.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_COVERED
		thumb.mouse_filter = Control.MOUSE_FILTER_IGNORE
		b.add_child(thumb)
		var text := Label.new()
		text.position = Vector2(356, 10)
		text.size = Vector2(204, 190)
		text.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		text.add_theme_font_size_override("font_size", 22)
		text.mouse_filter = Control.MOUSE_FILTER_IGNORE
		b.add_child(text)
		if info.is_empty():
			text.text = "%02d\nNo data" % (slot + 1)
			b.disabled = not saving
		else:
			thumb.texture = App.texture(info.picture)
			var time := Time.get_datetime_string_from_unix_time(int(info.time), true)
			text.text = "%02d  %s\n%s\n%s: %s" % [slot + 1, time, info.chapter, info.speaker, info.text]
		b.pressed.connect(func():
			if saving:
				var error: String = App.player.save_to(App.save_path(slot))
				if error != "":
					push_error(error)
				_open_overlay(_saves(true, page))
			else:
				load_slot(slot))
		grid.add_child(b)
	return s


# The backlog.

func _backlog() -> Control:
	var s := Ui.screen(0.92)
	s.add_child(_header("Backlog", close_overlay))
	var scroll := ScrollContainer.new()
	scroll.position = Vector2(120, 130)
	scroll.size = Vector2(1680, 920)
	s.add_child(scroll)
	var list := VBoxContainer.new()
	list.custom_minimum_size = Vector2(1640, 0)
	list.add_theme_constant_override("separation", 22)
	scroll.add_child(list)
	var player := AudioStreamPlayer.new()
	player.bus = "Voice"
	s.add_child(player)
	for line in App.player.backlog():
		var row := HBoxContainer.new()
		var voice: Variant = line.voice
		var b := Ui.button("▶", func():
			player.stream = App.audio(voice)
			player.play(), 26)
		b.disabled = voice == null
		row.add_child(b)
		var text := Ui.label(("%s\n" % line.speaker if line.speaker != "" else "") + "\n".join(line.lines), 32)
		text.custom_minimum_size = Vector2(1500, 0)
		text.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		row.add_child(text)
		list.add_child(row)
	s.ready.connect(func():
		await get_tree().process_frame
		scroll.scroll_vertical = int(scroll.get_v_scroll_bar().max_value))
	return s


# The settings.

func _settings() -> Control:
	var s := Ui.screen(0.94)
	s.add_child(_header("Settings", func():
		App.save_profile()
		close_overlay()))
	var grid := GridContainer.new()
	grid.columns = 2
	grid.position = Vector2(160, 150)
	grid.add_theme_constant_override("h_separation", 40)
	grid.add_theme_constant_override("v_separation", 22)
	s.add_child(grid)
	for item in [
		["Master volume", "volume_master", 0.8], ["Music volume", "volume_music", 0.8],
		["Voice volume", "volume_voice", 0.8], ["Sound volume", "volume_sound", 0.8],
	]:
		_slider(grid, item[0], item[1], item[2], 0.0, 1.0, App.apply_audio)
	_slider(grid, "Text speed", "text_speed", 45.0, 10.0, 200.0, func(): pass)
	_slider(grid, "Auto wait (seconds)", "auto_delay", 1.5, 0.0, 6.0, func(): pass)
	if not App.mobile:
		_check(grid, "Full screen", "fullscreen", false, App.apply_window)
	_check(grid, "Skip unread text", "skip_unread", false, func(): pass)
	_check(grid, "Stop the voice on click", "voice_stops", true, func(): pass)
	_check(grid, "Open all chapters", "open_all", false, func(): pass)
	var names: PackedStringArray = App.player.get_names()
	grid.add_child(Ui.label("Heroine family name", 34))
	var family := LineEdit.new()
	family.text = names[0]
	family.custom_minimum_size = Vector2(500, 0)
	grid.add_child(family)
	grid.add_child(Ui.label("Heroine given name", 34))
	var given := LineEdit.new()
	given.text = names[1]
	grid.add_child(given)
	var apply_names := func(_t = ""):
		App.player.set_names(family.text, given.text)
	family.text_changed.connect(apply_names)
	given.text_changed.connect(apply_names)
	grid.add_child(Ui.label("Import folder", 34))
	grid.add_child(Ui.button("Change", func():
		App.set_setting("import_folder", "")
		show_setup(), 30))
	return s


func _slider(grid: GridContainer, title: String, key: String, default: float, low: float, high: float, changed: Callable) -> void:
	grid.add_child(Ui.label(title, 34))
	var slider := HSlider.new()
	slider.min_value = low
	slider.max_value = high
	slider.step = (high - low) / 100.0
	slider.value = App.setting(key, default)
	slider.custom_minimum_size = Vector2(600, 40)
	slider.value_changed.connect(func(v: float):
		App.set_setting(key, v)
		changed.call())
	grid.add_child(slider)


func _check(grid: GridContainer, title: String, key: String, default: bool, changed: Callable) -> void:
	grid.add_child(Ui.label(title, 34))
	var box := CheckBox.new()
	box.button_pressed = App.setting(key, default)
	box.toggled.connect(func(on: bool):
		App.set_setting(key, on)
		changed.call())
	grid.add_child(box)


# The gallery and the music room.

func show_gallery() -> void:
	var s := Ui.screen(1.0)
	s.add_child(_header("Gallery: " + GAMES[App.game], show_title))
	var scroll := ScrollContainer.new()
	scroll.position = Vector2(80, 130)
	scroll.size = Vector2(1760, 920)
	s.add_child(scroll)
	var grid := GridContainer.new()
	grid.columns = 5
	grid.add_theme_constant_override("h_separation", 16)
	grid.add_theme_constant_override("v_separation", 16)
	scroll.add_child(grid)
	var game_number := 0 if App.game == "ve" else 1
	for image in App.player.gallery():
		if int(image.game) != game_number:
			continue
		var b := Button.new()
		b.custom_minimum_size = Vector2(336, 189)
		var thumb := TextureRect.new()
		thumb.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
		thumb.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
		thumb.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_COVERED
		thumb.mouse_filter = Control.MOUSE_FILTER_IGNORE
		b.add_child(thumb)
		var seen: bool = image.seen or App.setting("open_all", false)
		if seen:
			var file: String = image.file
			b.pressed.connect(func(): _view_image(file))
			b.ready.connect(func(): thumb.texture = App.texture(file))
		else:
			b.disabled = true
		grid.add_child(b)
	_switch(s)


func _view_image(file: String) -> void:
	var s := Ui.screen(1.0)
	var image := TextureRect.new()
	image.texture = App.texture(file)
	image.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	image.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	image.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT
	s.add_child(image)
	var close := Button.new()
	close.flat = true
	close.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	close.pressed.connect(close_overlay)
	s.add_child(close)
	_open_overlay(s)


func show_music() -> void:
	var s := Ui.screen(1.0)
	s.add_child(_header("Music", func():
		show_title()))
	var player := AudioStreamPlayer.new()
	player.bus = "Music"
	s.add_child(player)
	var scroll := ScrollContainer.new()
	scroll.position = Vector2(80, 130)
	scroll.size = Vector2(1760, 920)
	s.add_child(scroll)
	var grid := GridContainer.new()
	grid.columns = 6
	grid.add_theme_constant_override("h_separation", 12)
	grid.add_theme_constant_override("v_separation", 12)
	scroll.add_child(grid)
	for track in App.player.music():
		var t: Dictionary = track
		var b := Ui.button("BGM %02d" % int(t.number), func():
			var stream := App.audio(t.file)
			stream.loop = true
			stream.loop_offset = float(t.loop_start) / maxf(float(t.sample_rate), 1.0)
			player.stream = stream
			player.play(), 30)
		b.custom_minimum_size = Vector2(270, 70)
		grid.add_child(b)
	_switch(s)
