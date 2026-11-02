## The data of the whole program: the player of the Rust library, the
## settings, and the profile. Each screen reads them here.
extends Node

const SETTINGS_FILE := "user://settings.cfg"
const PROFILE_FILE := "user://profile.json"
const SAVE_SLOTS := 60

var player := DlPlayer.new()
var settings := ConfigFile.new()
## The full path of the import folder, with a "/" at the end.
var root := ""
## The game of the title screen: "ve" or "mb".
var game := "ve"


func _ready() -> void:
	settings.load(SETTINGS_FILE)
	var text := FileAccess.get_file_as_string(PROFILE_FILE)
	if text != "":
		player.set_profile(text)
	apply_audio()
	apply_window()


## Gives a setting. A missing setting gives the default.
func setting(key: String, default: Variant) -> Variant:
	return settings.get_value("player", key, default)


func set_setting(key: String, value: Variant) -> void:
	settings.set_value("player", key, value)
	settings.save(SETTINGS_FILE)


## Opens the import folder of the settings. Gives an empty text, or the
## error.
func open_import(folder: String = "") -> String:
	if folder == "":
		folder = setting("import_folder", "")
	if folder == "":
		return "no import folder"
	var error: String = player.open(folder)
	if error == "":
		root = player.root()
		set_setting("import_folder", folder)
	return error


func save_profile() -> void:
	var file := FileAccess.open(PROFILE_FILE, FileAccess.WRITE)
	if file:
		file.store_string(player.get_profile())


## Gives the full path of a file of the import folder.
func path(file: String) -> String:
	return root + file


var _textures := {}
var _images := {}


## Loads an image of the import folder. The images stay in a small cache.
func image(file: String) -> Image:
	if file == "":
		return null
	if _images.has(file):
		return _images[file]
	var img := Image.load_from_file(path(file))
	if img == null:
		push_warning("cannot load " + file)
		return null
	if _images.size() > 24:
		_images.clear()
	_images[file] = img
	return img


## Loads an image of the import folder as a texture. The textures stay in
## a small cache.
func texture(file: String) -> Texture2D:
	if file == "":
		return null
	if _textures.has(file):
		return _textures[file]
	var img := image(file)
	if img == null:
		return null
	var tex := ImageTexture.create_from_image(img)
	if _textures.size() > 64:
		_textures.clear()
	_textures[file] = tex
	return tex


## Loads an Ogg Vorbis file of the import folder.
func audio(file: String) -> AudioStreamOggVorbis:
	if file == "":
		return null
	return AudioStreamOggVorbis.load_from_file(path(file))


func save_path(slot: int) -> String:
	return ProjectSettings.globalize_path("user://saves/%s/%02d.json" % [game, slot])


func apply_audio() -> void:
	for bus in ["Master", "Music", "Voice", "Sound"]:
		var index := AudioServer.get_bus_index(bus)
		if index >= 0:
			var volume: float = setting("volume_" + bus.to_lower(), 0.8)
			AudioServer.set_bus_volume_db(index, linear_to_db(maxf(volume, 0.0001)))


func apply_window() -> void:
	if setting("fullscreen", false):
		DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_FULLSCREEN)
	else:
		DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_WINDOWED)
