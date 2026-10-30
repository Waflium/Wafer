use crate::fault::Frame;

pub const TAG_CODEC: &[Frame] = &[
    Frame::Fixed("net.minecraft.network.codec.ByteBufCodecs$18.encode(ByteBufCodecs.java:309)"),
    Frame::Fixed("net.minecraft.network.codec.ByteBufCodecs$18.encode(ByteBufCodecs.java:294)"),
];

pub const FROM_CODEC: &[Frame] = &[
    Frame::Fixed("net.minecraft.network.codec.ByteBufCodecs$18.encode(ByteBufCodecs.java:309)"),
    Frame::Fixed("net.minecraft.network.codec.ByteBufCodecs$18.encode(ByteBufCodecs.java:294)"),
    Frame::Fixed("net.minecraft.network.codec.ByteBufCodecs$20.encode(ByteBufCodecs.java:382)"),
    Frame::Fixed("net.minecraft.network.codec.ByteBufCodecs$20.encode(ByteBufCodecs.java:370)"),
];

pub const COMPOUND_TAG_CODEC: &[Frame] = &[
    Frame::Fixed("net.minecraft.network.codec.ByteBufCodecs$18.encode(ByteBufCodecs.java:309)"),
    Frame::Fixed("net.minecraft.network.codec.ByteBufCodecs$18.encode(ByteBufCodecs.java:294)"),
    Frame::Fixed("net.minecraft.network.codec.ByteBufCodecs$19.encode(ByteBufCodecs.java:351)"),
    Frame::Fixed("net.minecraft.network.codec.ByteBufCodecs$19.encode(ByteBufCodecs.java:341)"),
];

pub const OPTIONAL_COMPOUND_TAG: &[Frame] = &[
    Frame::Fixed("net.minecraft.network.codec.ByteBufCodecs$21.encode(ByteBufCodecs.java:395)"),
    Frame::Fixed("net.minecraft.network.codec.ByteBufCodecs$21.encode(ByteBufCodecs.java:387)"),
];

pub const OPTIONAL: &[Frame] = &[
    Frame::Fixed("net.minecraft.network.codec.ByteBufCodecs$25.encode(ByteBufCodecs.java:449)"),
    Frame::Fixed("net.minecraft.network.codec.ByteBufCodecs$25.encode(ByteBufCodecs.java:436)"),
];

pub const COLLECTION: &[Frame] = &[
    Frame::Fixed("net.minecraft.network.codec.ByteBufCodecs$26.encode(ByteBufCodecs.java:492)"),
    Frame::Fixed("net.minecraft.network.codec.ByteBufCodecs$26.encode(ByteBufCodecs.java:477)"),
];

pub const MAP: &[Frame] = &[Frame::Fixed("net.minecraft.network.codec.StreamCodec$4.encode(StreamCodec.java:87)")];

pub const OF_MEMBER: &[Frame] = &[Frame::Fixed("net.minecraft.network.codec.StreamCodec$2.encode(StreamCodec.java:48)")];

pub const OF: &[Frame] = &[Frame::Fixed("net.minecraft.network.codec.StreamCodec$1.encode(StreamCodec.java:33)")];

pub const MAP_STREAM: &[Frame] = &[
    Frame::Fixed("net.minecraft.network.codec.StreamCodec$5.encode(StreamCodec.java:103)"),
    Frame::Fixed("net.minecraft.network.codec.StreamCodec$5.encode(StreamCodec.java:93)"),
];

pub const ID_DISPATCH: &[Frame] = &[Frame::Fixed("net.minecraft.network.codec.IdDispatchCodec.encode(IdDispatchCodec.java:56)")];

pub const DISPATCH: &[Frame] = &[Frame::Fixed("net.minecraft.network.codec.StreamCodec$6.encode(StreamCodec.java:123)")];

pub const MAX_COMPOSITE_SIZE: usize = 14;

pub const fn composite(size: u8, field: u8) -> Frame {
    Frame::composite(size, field)
}
