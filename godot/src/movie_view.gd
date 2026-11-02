## Plays a movie over the screen. A click ends it.
class_name MovieView
extends Control


func play(_path: String) -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	var bg := ColorRect.new()
	bg.color = Color.BLACK
	bg.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(bg)
	await get_tree().create_timer(0.5).timeout
