## The theme and small helpers that the screens share.
class_name Ui
extends RefCounted

const GOLD := Color("c9a55a")
const INK := Color("120608")
const PAPER := Color("ece2cc")
const BLOOD := Color("5c0a12")


## Gives the font. The game font is a bitmap font of the console, so the
## player uses a font of the system that has Japanese characters.
static func font() -> Font:
	var f := SystemFont.new()
	f.font_names = PackedStringArray([
		"Hiragino Mincho ProN", "Yu Mincho", "Noto Serif CJK JP", "Noto Serif JP",
		"Hiragino Sans", "Yu Gothic", "Meiryo", "Noto Sans CJK JP", "Noto Sans JP", "serif",
	])
	return f


static func _box(fill: Color, border: Color, width: int) -> StyleBoxFlat:
	var s := StyleBoxFlat.new()
	s.bg_color = fill
	s.border_color = border
	s.set_border_width_all(width)
	s.set_corner_radius_all(4)
	s.content_margin_left = 24
	s.content_margin_right = 24
	s.content_margin_top = 10
	s.content_margin_bottom = 10
	return s


static func theme() -> Theme:
	var t := Theme.new()
	t.default_font = font()
	t.default_font_size = 40
	t.set_stylebox("normal", "Button", _box(Color(INK, 0.85), Color(GOLD, 0.6), 2))
	t.set_stylebox("hover", "Button", _box(Color(BLOOD, 0.9), GOLD, 2))
	t.set_stylebox("pressed", "Button", _box(Color(BLOOD, 1.0), PAPER, 2))
	t.set_stylebox("focus", "Button", _box(Color(0, 0, 0, 0), PAPER, 2))
	t.set_stylebox("disabled", "Button", _box(Color(INK, 0.5), Color(GOLD, 0.2), 2))
	t.set_color("font_color", "Button", PAPER)
	t.set_color("font_hover_color", "Button", Color.WHITE)
	t.set_color("font_disabled_color", "Button", Color(PAPER, 0.35))
	t.set_stylebox("panel", "Panel", _box(Color(INK, 0.92), Color(GOLD, 0.5), 2))
	t.set_stylebox("panel", "PanelContainer", _box(Color(INK, 0.92), Color(GOLD, 0.5), 2))
	t.set_color("font_color", "Label", PAPER)
	t.set_color("default_color", "RichTextLabel", PAPER)
	t.set_stylebox("normal", "LineEdit", _box(Color(0, 0, 0, 0.6), Color(GOLD, 0.6), 2))
	t.set_color("font_color", "LineEdit", PAPER)
	return t


static func label(text: String, font_size: int = 40, color: Color = PAPER) -> Label:
	var l := Label.new()
	l.text = text
	l.add_theme_font_size_override("font_size", font_size)
	l.add_theme_color_override("font_color", color)
	return l


static func button(text: String, action: Callable, font_size: int = 40) -> Button:
	var b := Button.new()
	b.text = text
	b.add_theme_font_size_override("font_size", font_size)
	b.pressed.connect(action)
	return b


## Gives a full screen control with a dark background.
static func screen(dim: float = 0.9) -> Control:
	var c := Control.new()
	c.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	var bg := ColorRect.new()
	bg.color = Color(INK, dim)
	bg.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	c.add_child(bg)
	return c


## Gives a texture region of a UI texture of the import folder.
static func region(file: String, rect: Rect2) -> AtlasTexture:
	var a := AtlasTexture.new()
	a.atlas = App.texture(file)
	a.region = rect
	return a
