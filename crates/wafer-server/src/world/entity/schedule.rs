use wafer_data::registry;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Activity(pub u16);

impl Activity {
    pub const CORE: Activity = Activity(0);
    pub const IDLE: Activity = Activity(1);
    pub const WORK: Activity = Activity(2);
    pub const PLAY: Activity = Activity(3);
    pub const REST: Activity = Activity(4);
    pub const MEET: Activity = Activity(5);
    pub const PANIC: Activity = Activity(6);
    pub const RAID: Activity = Activity(7);
    pub const PRE_RAID: Activity = Activity(8);
    pub const HIDE: Activity = Activity(9);
    pub const FIGHT: Activity = Activity(10);
    pub const CELEBRATE: Activity = Activity(11);
    pub const ADMIRE_ITEM: Activity = Activity(12);
    pub const AVOID: Activity = Activity(13);
    pub const RIDE: Activity = Activity(14);
    pub const PLAY_DEAD: Activity = Activity(15);
    pub const LONG_JUMP: Activity = Activity(16);
    pub const RAM: Activity = Activity(17);
    pub const TONGUE: Activity = Activity(18);
    pub const SWIM: Activity = Activity(19);
    pub const LAY_SPAWN: Activity = Activity(20);
    pub const SNIFF: Activity = Activity(21);
    pub const INVESTIGATE: Activity = Activity(22);
    pub const ROAR: Activity = Activity(23);
    pub const EMERGE: Activity = Activity(24);
    pub const DIG: Activity = Activity(25);

    pub fn by_name(name: &str) -> Option<Activity> {
        registry::ACTIVITY.iter().position(|&a| a == name).map(|i| Activity(i as u16))
    }

    pub fn name(self) -> &'static str {
        registry::ACTIVITY[self.0 as usize]
    }

    pub fn get_name(self) -> &'static str {
        self.name().trim_start_matches("minecraft:")
    }

    pub fn hash_code(self) -> i32 {
        wafer_core::random::java_string_hash(self.get_name())
    }
}
