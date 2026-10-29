use wafer_core::random::RandomSource;
use wafer_core::{Axis, BlockPos, mth};
use serde_json::Value;
use wafer_protocol::nbt::Nbt;

use crate::world::item::component::codec::{self, Codec, Dispatch, Field, Selector, Stream};
use crate::world::item::component::types::{self, Pair, def};

fn linear_fields() -> Vec<Field> {
    vec![
        def("min_chance", types::float(Codec::float()), Nbt::Float(0.0)),
        def("max_chance", types::float(Codec::float()), Nbt::Float(0.0)),
        def("min_dist", (Codec::int(), Stream::Int), Nbt::Int(0)),
        def("max_dist", (Codec::int(), Stream::Int), Nbt::Int(0)),
    ]
}

pub fn codec() -> Pair {
    let mut axis_aligned = linear_fields();
    axis_aligned.push(def("axis", types::enumeration(&["x", "y", "z"]), Nbt::String("y".into())));
    let cases = vec![
        ("minecraft:always_true", vec![]),
        ("minecraft:linear_pos", linear_fields()),
        ("minecraft:axis_aligned_linear_pos", axis_aligned),
    ];
    let dispatch = Codec::Dispatch(Box::new(Dispatch { key: "predicate_type", selector: Selector::Registry("minecraft:pos_rule_test"), cases }));
    (Codec::Normalize(Box::new(dispatch), check_range), Stream::Dispatch)
}

fn check_range(test: Nbt) -> Result<Nbt, String> {
    let head: &[&str] = match test.get("predicate_type").and_then(Nbt::as_str) {
        Some("minecraft:linear_pos") => LINEAR_POS_TEST_FRAMES,
        Some("minecraft:axis_aligned_linear_pos") => AXIS_ALIGNED_LINEAR_POS_TEST_FRAMES,
        _ => return Ok(test),
    };
    let dist = |name: &str| test.get(name).map_or(0, codec::as_i32);
    let (min_dist, max_dist) = (dist("min_dist"), dist("max_dist"));
    if min_dist >= max_dist {
        let frames = [head, DISPATCH_FRAMES, POSITION_PREDICATE_FIELD_FRAMES].concat();
        crate::core::data_registries::throw("java.lang.IllegalArgumentException", format!("Invalid range: [{min_dist},{max_dist}]"), &frames);
    }
    Ok(test)
}

const LINEAR_POS_TEST_FRAMES: &[&str] = &[
    "net.minecraft.world.level.levelgen.structure.templatesystem.LinearPosTest.<init>(LinearPosTest.java:25)",
    "com.mojang.datafixers.util.Function4.lambda$curry2$2(Function4.java:14)",
    "com.mojang.serialization.DataResult$Instance.ap2(DataResult.java:434)",
    "com.mojang.datafixers.kinds.Applicative.ap4(Applicative.java:84)",
    "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$7.decode(RecordCodecBuilder.java:404)",
    "com.mojang.serialization.codecs.RecordCodecBuilder$2.decode(RecordCodecBuilder.java:107)",
];

const AXIS_ALIGNED_LINEAR_POS_TEST_FRAMES: &[&str] = &[
    "net.minecraft.world.level.levelgen.structure.templatesystem.AxisAlignedLinearPosTest.<init>(AxisAlignedLinearPosTest.java:28)",
    "com.mojang.datafixers.util.Function5.lambda$curry2$2(Function5.java:14)",
    "com.mojang.serialization.DataResult$Instance.ap3(DataResult.java:456)",
    "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$5.decode(RecordCodecBuilder.java:321)",
    "com.mojang.serialization.codecs.RecordCodecBuilder$2.decode(RecordCodecBuilder.java:107)",
];

const DISPATCH_FRAMES: &[&str] = &[
    "com.mojang.serialization.codecs.KeyDispatchCodec.lambda$decode$2(KeyDispatchCodec.java:46)",
    "com.mojang.serialization.DataResult$Success.flatMap(DataResult.java:201)",
    "com.mojang.serialization.codecs.KeyDispatchCodec.lambda$decode$3(KeyDispatchCodec.java:38)",
    "com.mojang.serialization.DataResult$Success.flatMap(DataResult.java:201)",
    "com.mojang.serialization.codecs.KeyDispatchCodec.decode(KeyDispatchCodec.java:37)",
    "com.mojang.serialization.MapDecoder.lambda$compressedDecode$1(MapDecoder.java:52)",
    "com.mojang.serialization.DataResult$Success.flatMap(DataResult.java:201)",
    "com.mojang.serialization.MapDecoder.compressedDecode(MapDecoder.java:52)",
    "com.mojang.serialization.MapCodec$MapCodecCodec.decode(MapCodec.java:159)",
];

const POSITION_PREDICATE_FIELD_FRAMES: &[&str] = &[
    "com.mojang.serialization.Decoder.parse(Decoder.java:18)",
    "com.mojang.serialization.codecs.OptionalFieldCodec.decode(OptionalFieldCodec.java:34)",
    "com.mojang.serialization.MapDecoder$4.decode(MapDecoder.java:94)",
    "com.mojang.serialization.MapCodec$2.decode(MapCodec.java:78)",
    "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$5.decode(RecordCodecBuilder.java:323)",
    "com.mojang.serialization.codecs.RecordCodecBuilder$2.decode(RecordCodecBuilder.java:107)",
];

pub enum PosRuleTest {
    AlwaysTrue,
    LinearPos { min_chance: f32, max_chance: f32, min_dist: i32, max_dist: i32 },
    AxisAlignedLinearPos { min_chance: f32, max_chance: f32, min_dist: i32, max_dist: i32, axis: Axis },
}

impl PosRuleTest {
    pub fn parse(json: &Value) -> Result<PosRuleTest, String> {
        let ty = json.get("predicate_type").and_then(Value::as_str).ok_or("pos rule test without predicate_type")?;
        let float = |key: &str| json.get(key).and_then(Value::as_f64).unwrap_or(0.0) as f32;
        let int = |key: &str| json.get(key).and_then(Value::as_i64).unwrap_or(0) as i32;
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "always_true" => PosRuleTest::AlwaysTrue,
            "linear_pos" => PosRuleTest::LinearPos { min_chance: float("min_chance"), max_chance: float("max_chance"), min_dist: int("min_dist"), max_dist: int("max_dist") },
            "axis_aligned_linear_pos" => PosRuleTest::AxisAlignedLinearPos {
                min_chance: float("min_chance"),
                max_chance: float("max_chance"),
                min_dist: int("min_dist"),
                max_dist: int("max_dist"),
                axis: match json.get("axis").and_then(Value::as_str).unwrap_or("y") {
                    "x" => Axis::X,
                    "z" => Axis::Z,
                    _ => Axis::Y,
                },
            },
            other => return Err(format!("unknown pos rule test {other}")),
        })
    }

    pub fn test(&self, _in_template_pos: BlockPos, world_pos: BlockPos, world_reference: BlockPos, random: &mut impl RandomSource) -> bool {
        match *self {
            PosRuleTest::AlwaysTrue => true,
            PosRuleTest::LinearPos { min_chance, max_chance, min_dist, max_dist } => {
                let dist = (world_pos.x - world_reference.x).abs() + (world_pos.y - world_reference.y).abs() + (world_pos.z - world_reference.z).abs();
                let rnd = random.next_float();
                rnd <= mth::clamped_lerp_f(mth::inverse_lerp_f(dist as f32, min_dist as f32, max_dist as f32), min_chance, max_chance)
            }
            PosRuleTest::AxisAlignedLinearPos { min_chance, max_chance, min_dist, max_dist, axis } => {
                let (step_x, step_y, step_z) = match axis {
                    Axis::X => (1, 0, 0),
                    Axis::Y => (0, 1, 0),
                    Axis::Z => (0, 0, 1),
                };
                let xd = ((world_pos.x - world_reference.x) * step_x).abs() as f32;
                let yd = ((world_pos.y - world_reference.y) * step_y).abs() as f32;
                let zd = ((world_pos.z - world_reference.z) * step_z).abs() as f32;
                let dist = (xd + yd + zd) as i32;
                let rnd = random.next_float();
                rnd <= mth::clamped_lerp_f(mth::inverse_lerp_f(dist as f32, min_dist as f32, max_dist as f32), min_chance, max_chance)
            }
        }
    }
}
