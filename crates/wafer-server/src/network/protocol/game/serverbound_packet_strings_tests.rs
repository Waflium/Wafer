use super::serverbound_packet_strings::to_string;
use crate::network::connection::Packet;

const VANILLA: &[(&str, &str)] = &[
    ("120003fc19ff0506000000000200ffff0000240101050103000004d20101000501f4070100000011000101400000", "ServerboundContainerClickPacket[containerId=0, stateId=3, slotNum=-999, buttonNum=-1, containerInput=QUICK_CRAFT, changedSlots={0=><empty>, -1=><empty>, 17=><empty>, 2=><empty>, 36=>ActualItem[item=Reference{ResourceKey[minecraft:item / minecraft:stone]=minecraft:stone}, count=5, components=HashedPatchMap[addedComponents={minecraft:damage=1234}, removedComponents=[minecraft:max_stack_size]]], 5=>ActualItem[item=Reference{ResourceKey[minecraft:item / minecraft:diamond]=minecraft:diamond}, count=1, components=HashedPatchMap[addedComponents={}, removedComponents=[]]]}, carriedItem=ActualItem[item=Reference{ResourceKey[minecraft:item / minecraft:stone]=minecraft:stone}, count=64, components=HashedPatchMap[addedComponents={}, removedComponents=[]]]]"),
    ("3501120109", "ServerboundSetBeaconPacket[primary=Optional[Reference{ResourceKey[minecraft:mob_effect / minecraft:poison]=net.minecraft.world.effect.PoisonMobEffect@HASH}], secondary=Optional[Reference{ResourceKey[minecraft:mob_effect / minecraft:regeneration]=net.minecraft.world.effect.RegenerationMobEffect@HASH}]]"),
    ("090178ffffffffffffffff0000000000000000000000000000", "ServerboundChatPacket[message=x, timeStamp=1969-12-31T23:59:59.999Z, salt=0, signature=Optional.empty, lastSeenMessages=Update[offset=0, acknowledged={}, checksum=0]]"),
    ("0901787fffffffffffffff0000000000000000000000000000", "ServerboundChatPacket[message=x, timeStamp=+292278994-08-17T07:12:55.807Z, salt=0, signature=Optional.empty, lastSeenMessages=Update[offset=0, acknowledged={}, checksum=0]]"),
    ("09017880000000000000000000000000000000000000000000", "ServerboundChatPacket[message=x, timeStamp=-292275055-05-16T16:47:04.192Z, salt=0, signature=Optional.empty, lastSeenMessages=Update[offset=0, acknowledged={}, checksum=0]]"),
    ("090178ffffc77590fb9c180000000000000000000000000000", "ServerboundChatPacket[message=x, timeStamp=-0001-12-31T23:59:59Z, salt=0, signature=Optional.empty, lastSeenMessages=Update[offset=0, acknowledged={}, checksum=0]]"),
    ("0901780000e677d21fdc000000000000000000000000000000", "ServerboundChatPacket[message=x, timeStamp=+10000-01-01T00:00:00Z, salt=0, signature=Optional.empty, lastSeenMessages=Update[offset=0, acknowledged={}, checksum=0]]"),
    ("090178fffea87aacaeec000000000000000000000000000000", "ServerboundChatPacket[message=x, timeStamp=-9999-01-01T00:00:00Z, salt=0, signature=Optional.empty, lastSeenMessages=Update[offset=0, acknowledged={}, checksum=0]]"),
    ("00073ff8000000000000c04e0000000000003fb999999999999a42b40000c1480000", "ServerboundAcceptTeleportationPacket[id=7, x=1.5, y=-60.0, z=0.1, yRot=90.0, xRot=-12.5]"),
    ("012a", "ServerboundAttackPacket[entityId=42]"),
    ("0303ffffffff0f", "ServerboundSelectBundleItemPacket[slotId=3, selectedItemIndex=-1]"),
    ("0403", "ServerboundChangeDifficultyPacket[difficulty=HARD]"),
    ("0503", "ServerboundChangeGameModePacket[mode=SPECTATOR]"),
    ("0605", "ServerboundChatAckPacket[offset=5]"),
    ("0706736179206869", "ServerboundChatCommandPacket[command=say hi]"),
    ("080f6d73672053746576652068656c6c6f000001a189ec5789fffffffff8a432eb01076d657373616765030a11181f262d343b424950575e656c737a81888f969da4abb2b9c0c7ced5dce3eaf1f8ff060d141b222930373e454c535a61686f767d848b9299a0a7aeb5bcc3cad1d8dfe6edf4fb020910171e252c333a41484f565d646b727980878e959ca3aab1b8bfc6cdd4dbe2e9f0f7fe050c131a21282f363d444b525960676e757c838a91989fa6adb4bbc2c9d0d7dee5ecf3fa01080f161d242b323940474e555c636a71787f868d949ba2a9b0b7bec5ccd3dae1e8eff6fd040b121920272e353c434a51585f666d747b828990979ea5acb3bac1c8cfd6dde4ebf2f900070e151c232a31383f464d545b626970777e858c939aa1a8afb6bdc4cbd2d9e0e7eef5fc02090008fb", "ServerboundChatCommandSignedPacket[command=msg Steve hello, timeStamp=2026-10-29T23:08:49.929Z, salt=-123456789, argumentSignatures=ArgumentSignatures[entries=[Entry[name=message, signature=AwoRGB8mLTQ7QklQV15lbHN6gYiPlp2kq7K5wMfO1dzj6vH4/wYNFBsiKTA3PkVMU1phaG92fYSLkpmgp661vMPK0djf5u30+wIJEBceJSwzOkFIT1ZdZGtyeYCHjpWco6qxuL/GzdTb4unw9/4FDBMaISgvNj1ES1JZYGdudXyDipGYn6attLvCydDX3uXs8/oBCA8WHSQrMjlAR05VXGNqcXh/ho2Um6KpsLe+xczT2uHo7/b9BAsSGSAnLjU8Q0pRWF9mbXR7gomQl56lrLO6wcjP1t3k6/L5AAcOFRwjKjE4P0ZNVFtiaXB3foWMk5qhqK+2vcTL0tng5+71/A==]]], lastSeenMessages=Update[offset=2, acknowledged={0, 3, 19}, checksum=-5]]"),
    ("08057361792078000001a189ec53e80000000000000000000000000000", "ServerboundChatCommandSignedPacket[command=say x, timeStamp=2026-10-29T23:08:49Z, salt=0, argumentSignatures=ArgumentSignatures[entries=[]], lastSeenMessages=Update[offset=0, acknowledged={}, checksum=0]]"),
    ("090568656c6c6f000001a189ec5789000000000000006301030a11181f262d343b424950575e656c737a81888f969da4abb2b9c0c7ced5dce3eaf1f8ff060d141b222930373e454c535a61686f767d848b9299a0a7aeb5bcc3cad1d8dfe6edf4fb020910171e252c333a41484f565d646b727980878e959ca3aab1b8bfc6cdd4dbe2e9f0f7fe050c131a21282f363d444b525960676e757c838a91989fa6adb4bbc2c9d0d7dee5ecf3fa01080f161d242b323940474e555c636a71787f868d949ba2a9b0b7bec5ccd3dae1e8eff6fd040b121920272e353c434a51585f666d747b828990979ea5acb3bac1c8cfd6dde4ebf2f900070e151c232a31383f464d545b626970777e858c939aa1a8afb6bdc4cbd2d9e0e7eef5fc02090008fb", "ServerboundChatPacket[message=hello, timeStamp=2026-10-29T23:08:49.929Z, salt=99, signature=Optional[AwoRGB8mLTQ7QklQV15lbHN6gYiPlp2kq7K5wMfO1dzj6vH4/wYNFBsiKTA3PkVMU1phaG92fYSLkpmgp661vMPK0djf5u30+wIJEBceJSwzOkFIT1ZdZGtyeYCHjpWco6qxuL/GzdTb4unw9/4FDBMaISgvNj1ES1JZYGdudXyDipGYn6attLvCydDX3uXs8/oBCA8WHSQrMjlAR05VXGNqcXh/ho2Um6KpsLe+xczT2uHo7/b9BAsSGSAnLjU8Q0pRWF9mbXR7gomQl56lrLO6wcjP1t3k6/L5AAcOFRwjKjE4P0ZNVFtiaXB3foWMk5qhqK+2vcTL0tng5+71/A==], lastSeenMessages=Update[offset=2, acknowledged={0, 3, 19}, checksum=-5]]"),
    ("0905706c61696e00000000000000000000000000000000000000000000", "ServerboundChatPacket[message=plain, timeStamp=1970-01-01T00:00:00Z, salt=0, signature=Optional.empty, lastSeenMessages=Update[offset=0, acknowledged={}, checksum=0]]"),
    ("0a0123456789abcdeffedcba9876543210000001a189ec5789a20130819f300d06092a864886f70d010101050003818d0030818902818100c3a1f00d1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdefdeadbeef020301000103010203", "ServerboundChatSessionUpdatePacket[chatSession=Data[sessionId=01234567-89ab-cdef-fedc-ba9876543210, profilePublicKey=Data[expiresAt=2026-10-29T23:08:49.929Z, key=Sun RSA public key, 1024 bits\n  params: null\n  modulus: 137377861912387485898773339111142322479295538990683442423815629175271519211090618621532883649916796627530339118877243405108286769687263939262550293597335503509493696782486415829349193638843545765197096343582251260679467036337907511553003719040380836253422138423108813061056915214448130560236243055469067615983\n  public exponent: 65537, keySignature=[B@HASH]]]"),
    ("0b41180000", "ServerboundChunkBatchReceivedPacket[desiredChunksPerTick=9.5]"),
    ("0b7fc00000", "ServerboundChunkBatchReceivedPacket[desiredChunksPerTick=NaN]"),
    ("0d", "ServerboundClientTickEndPacket[]"),
    ("0e05656e5f75730c01017f00000101", "ServerboundClientInformationPacket[information=ClientInformation[language=en_us, viewDistance=12, chatVisibility=SYSTEM, chatColors=true, modelCustomisation=127, mainHand=LEFT, textFilteringEnabled=false, allowsListing=true, particleStatus=DECREASED]]"),
    ("110102", "ServerboundContainerButtonClickPacket[containerId=1, buttonId=2]"),
    ("120311002400000000", "ServerboundContainerClickPacket[containerId=3, stateId=17, slotNum=36, buttonNum=0, containerInput=PICKUP, changedSlots={}, carriedItem=<empty>]"),
    ("14040101", "ServerboundContainerSlotStateChangedPacket[slotId=4, containerId=1, newState=true]"),
    ("150b6d696e6563726166743a63010109", "ServerboundCookieResponsePacket[key=minecraft:c, payload=[B@HASH]"),
    ("1503783a7900", "ServerboundCookieResponsePacket[key=x:y, payload=null]"),
    ("160f6d696e6563726166743a6272616e640776616e696c6c61", "ServerboundCustomPayloadPacket[payload=BrandPayload[brand=vanilla]]"),
    ("16086d6f643a64617461", "ServerboundCustomPayloadPacket[payload=DiscardedPayload[id=mod:data]]"),
    ("170101", "ServerboundDebugSubscriptionRequestPacket[subscriptions={minecraft:bees}]"),
    ("1700", "ServerboundDebugSubscriptionRequestPacket[subscriptions={}]"),
    ("1802020870616765206f6e650374776f01055469746c65", "ServerboundEditBookPacket[slot=2, pages=[page one, two], title=Optional[Title]]"),
    ("18000000", "ServerboundEditBookPacket[slot=0, pages=[], title=Optional.empty]"),
    ("1a0b01f97f4001fffa01", "ServerboundInteractPacket[entityId=11, hand=OFF_HAND, location=(0.2500152597204419, 1.0, -0.49996948055911616), usingSecondaryAction=true]"),
    ("2240240000000000004050000000000000c00a666666666666423400000000000001", "ServerboundMoveVehiclePacket[movingTo=Immutable[position=(10.0, 64.0, -3.3), yRot=45.0, xRot=0.0], onGround=true]"),
    ("240000004000002fc401", "ServerboundPickItemFromBlockPacket[pos=BlockPos{x=1, y=-60, z=2}, includeData=true]"),
    ("250800", "ServerboundPickItemFromEntityPacket[id=8, includeData=false]"),
    ("27014d01", "ServerboundPlaceRecipePacket[containerId=1, recipe=RecipeDisplayId[index=77], useMaxItems=true]"),
    ("2b55", "ServerboundPlayerInputPacket[input=Input[forward=true, backward=false, left=true, right=false, jump=true, shift=false, sprint=true]]"),
    ("2c", "ServerboundPlayerLoadedPacket[]"),
    ("2e", "ServerboundPunchPacket[]"),
    ("3003", "ServerboundRecipeBookSeenRecipePacket[recipe=RecipeDisplayId[index=3]]"),
    ("320000000000000001000000000000000200", "ServerboundResourcePackPacket[id=00000000-0000-0001-0000-000000000002, action=SUCCESSFULLY_LOADED]"),
    ("35010000", "ServerboundSetBeaconPacket[primary=Optional[Reference{ResourceKey[minecraft:mob_effect / minecraft:speed]=net.minecraft.world.effect.MobEffect@HASH}], secondary=Optional.empty]"),
    ("39002403f4070000", "ServerboundSetCreativeModeSlotPacket[slotNum=36, itemStack=3 minecraft:diamond]"),
    ("39ffff00", "ServerboundSetCreativeModeSlotPacket[slotNum=-1, itemStack=0 minecraft:air]"),
    ("3a01186d696e6563726166743a6b6565705f696e76656e746f72790474727565", "ServerboundSetGameRulePacket[entries=[Entry[gameRuleKey=ResourceKey[minecraft:game_rule / minecraft:keep_inventory], value=true]]]"),
    ("3d000000000000000001036d7367", "ServerboundSetTestBlockPacket[position=BlockPos{x=0, y=0, z=0}, mode=LOG, message=msg]"),
    ("3e00000140000070060161000163016400", "ServerboundSignUpdatePacket[pos=BlockPos{x=5, y=6, z=7}, lines=[a, , c, d], slot=BACK]"),
    ("3f05", "ServerboundSpectatorActionPacket[spectateEntityId=OptionalInt[4]]"),
    ("3f00", "ServerboundSpectatorActionPacket[spectateEntityId=OptionalInt.empty]"),
    ("41000000400000300201010b6d696e6563726166743a74010203010000010800046f6f7073", "ServerboundTestInstanceBlockActionPacket[pos=BlockPos{x=1, y=2, z=3}, action=QUERY, data=Data[test=Optional[ResourceKey[minecraft:test_instance / minecraft:t]], size=Vec3i{x=1, y=2, z=3}, rotation=CLOCKWISE_90, ignoreEntities=false, status=CLEARED, errorMessage=Optional[literal{oops}]]]"),
    ("4100000000000000000000050505000102010a090004776974680a000000020300000000000100080000000178000800097472616e736c6174650003612e6200", "ServerboundTestInstanceBlockActionPacket[pos=BlockPos{x=0, y=0, z=0}, action=INIT, data=Data[test=Optional.empty, size=Vec3i{x=5, y=5, z=5}, rotation=NONE, ignoreEntities=true, status=FINISHED, errorMessage=Optional[translation{key='a.b', args=[1, x]}]]]"),
    ("42000000004000003001013f0000003f8000003e80000000000c", "ServerboundUseItemOnPacket[hand=MAIN_HAND, hitResult=net.minecraft.world.phys.BlockHitResult@HASH, sequence=12]"),
    ("43010343340000c2b40000", "ServerboundUseItemPacket[hand=OFF_HAND, sequence=3, yRot=180.0, xRot=-90.0]"),
    ("4405783a616374130a030001610000000108000162000378207900", "ServerboundCustomClickActionPacket[id=x:act, payload=Optional[{a:1,b:\"x y\"}]]"),
    ("440e6d696e6563726166743a6e6f6e650100", "ServerboundCustomClickActionPacket[id=minecraft:none, payload=Optional.empty]"),
];

fn without_identity_hashes(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find('@') {
        out.push_str(&rest[..=at]);
        out.push_str("HASH");
        rest = rest[at + 1..].trim_start_matches(|c: char| c.is_ascii_hexdigit());
    }
    out.push_str(rest);
    out
}

#[test]
fn serverbound_packets_print_like_vanilla() {
    for (hex, expected) in VANILLA {
        let bytes: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect();
        let mut r = wafer_protocol::Reader::new(&bytes);
        let id = r.varint().unwrap();
        let packet = Packet { id, data: bytes[bytes.len() - r.remaining()..].to_vec() };
        assert_eq!(without_identity_hashes(&to_string(&packet).unwrap_or_else(|| panic!("{hex}"))), *expected);
    }
}
