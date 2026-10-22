#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemDisplayContext {
    None,
    ThirdPersonLeftHand,
    ThirdPersonRightHand,
    FirstPersonLeftHand,
    FirstPersonRightHand,
    Head,
    Gui,
    Ground,
    Fixed,
    OnShelf,
}

pub const VALUES: [(&str, ItemDisplayContext); 10] = [
    ("none", ItemDisplayContext::None),
    ("thirdperson_lefthand", ItemDisplayContext::ThirdPersonLeftHand),
    ("thirdperson_righthand", ItemDisplayContext::ThirdPersonRightHand),
    ("firstperson_lefthand", ItemDisplayContext::FirstPersonLeftHand),
    ("firstperson_righthand", ItemDisplayContext::FirstPersonRightHand),
    ("head", ItemDisplayContext::Head),
    ("gui", ItemDisplayContext::Gui),
    ("ground", ItemDisplayContext::Ground),
    ("fixed", ItemDisplayContext::Fixed),
    ("on_shelf", ItemDisplayContext::OnShelf),
];

impl ItemDisplayContext {
    pub fn get_id(self) -> i8 {
        self as i8
    }

    pub fn by_id(id: i8) -> ItemDisplayContext {
        VALUES.get(id as usize).map_or(ItemDisplayContext::None, |(_, context)| *context)
    }

    pub fn get_serialized_name(self) -> &'static str {
        VALUES[self as usize].0
    }
}
