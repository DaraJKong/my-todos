use xilem::Color;
use xilem::masonry::core::DefaultProperties;
use xilem::masonry::widgets::TextInput;
use xilem::style::Background;

pub const BACKGROUND_COLOR: Color = Color::from_rgb8(0x0a, 0x0a, 0x0a);
pub const SURFACE_COLOR: Color = Color::from_rgb8(0x14, 0x14, 0x14);
pub const SURFACE_BORDER_COLOR: Color = Color::from_rgb8(0x1e, 0x1e, 0x1e);

pub const SUCCESS_COLOR: Color = Color::from_rgb8(0x37, 0xc8, 0x37);
pub const DANGER_COLOR: Color = Color::from_rgb8(0xc8, 0x37, 0x37);

pub fn apply_theme(def_props: &mut DefaultProperties) {
    def_props.insert::<TextInput, Background>(Background::Color(
        SURFACE_COLOR.map_lightness(|l| l * 0.95),
    ));
}
