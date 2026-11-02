## One character sprite: the body, an eye patch, and a mouth patch from one
## sheet. The eyes blink. The mouth moves with the lip sync keys of a
## voice.
class_name Figure
extends Control

var _layout: Dictionary
var _expression: Dictionary
var _sheet: Image
## The textures of the parts of the sheet, by their rectangle.
var _parts := {}
var _body := TextureRect.new()
var _eyes := TextureRect.new()
var _mouth := TextureRect.new()
var _blink_time := 0.0
var _blink_frame := 0
## The lip sync keys: pairs of a sample position and a state.
var _lip: Array = []
var _lip_rate := 44100.0
var _voice: AudioStreamPlayer


func _init() -> void:
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	for part in [_body, _eyes, _mouth]:
		part.mouse_filter = Control.MOUSE_FILTER_IGNORE
		add_child(part)
	_blink_time = randf_range(2.0, 5.0)


## Sets the sheet and the expression from an event of kind "figure" or
## "look".
func set_look(event: Dictionary) -> void:
	_layout = event.layout
	_expression = _layout.expressions[int(event.expression)]
	if _sheet != App.image(event.file):
		_parts.clear()
	_sheet = App.image(event.file)
	var body: Dictionary = _layout.body
	size = Vector2(body.x, body.y)
	_body.texture = _atlas(Vector2.ZERO, size)
	_eyes.position = Vector2(_layout.eye_at.x, _layout.eye_at.y)
	_mouth.position = Vector2(_layout.mouth_at.x, _layout.mouth_at.y)
	_set_eyes(0)
	_set_mouth(0)
	var tint: Array = event.get("tint", [-1, -1, -1])
	if int(tint[0]) >= 0:
		modulate = Color(tint[0] / 128.0, tint[1] / 128.0, tint[2] / 128.0, modulate.a)
	else:
		modulate = Color(1, 1, 1, modulate.a)


## Gives a part of the sheet as its own texture. A region of one large
## texture takes colour from the pixels next to it when the screen scales
## the sprite, and that shows a line around a patch.
func _atlas(at: Vector2, part_size: Vector2) -> Texture2D:
	var rect := Rect2i(at, part_size)
	if not _parts.has(rect):
		if _sheet == null:
			return null
		_parts[rect] = ImageTexture.create_from_image(_sheet.get_region(rect))
	return _parts[rect]


func _set_eyes(frame: int) -> void:
	if _expression.is_empty():
		return
	var at: Dictionary = _expression.eyes[frame]
	_eyes.texture = _atlas(Vector2(at.x, at.y), Vector2(_layout.eye_size.x, _layout.eye_size.y))


func _set_mouth(frame: int) -> void:
	if _expression.is_empty():
		return
	var at: Dictionary = _expression.mouths[frame]
	_mouth.texture = _atlas(Vector2(at.x, at.y), Vector2(_layout.mouth_size.x, _layout.mouth_size.y))


## Moves the mouth with a voice until the voice stops.
func speak(voice: AudioStreamPlayer, lip: Variant) -> void:
	_voice = voice
	_lip = []
	if lip is Dictionary:
		_lip = lip.keys
		_lip_rate = float(lip.sample_rate)


func silence() -> void:
	_voice = null
	_lip = []
	_set_mouth(0)


func _process(delta: float) -> void:
	_blink_time -= delta
	if _blink_time <= 0.0:
		_blink_frame += 1
		if _blink_frame > 3:
			_blink_frame = 0
			_blink_time = randf_range(2.5, 6.0)
		else:
			_blink_time = 0.05
		_set_eyes([0, 1, 2, 1][_blink_frame])
	if _voice == null or _lip.is_empty():
		return
	if not _voice.playing:
		silence()
		return
	var sample := int(_voice.get_playback_position() * _lip_rate)
	var state := 0
	for key in _lip:
		if int(key[0]) > sample:
			break
		state = int(key[1])
	_set_mouth(2 if state == 1 else 0)
