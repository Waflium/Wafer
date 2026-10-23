use crate::datafixers::dsl::TypeReference;
use crate::datafixers::dynamic::Dynamic;
use crate::util::datafix::data_fixers::get_data_fixer;
use crate::util::datafix::schemas::{v1451_6, v3818_5, v704, v705};

fn update(ty: &'static str, from: i32, to: i32, input: &str) -> String {
    let input = crate::nbt::parse_fully(input).expect("valid snbt");
    let output = get_data_fixer().update(TypeReference(ty), Dynamic::new(input), from, to);
    crate::nbt::to_string(output.get_value().expect("a value"))
}

#[test]
fn equipment_splits_into_armor_and_hand_items() {
    assert_eq!(
        update("entity_chunk", 99, 100, r#"{Entities:[{id:"Zombie",Equipment:[{id:"stone",Count:1b},{id:"minecraft:iron_boots",Count:1b}],DropChances:[0.5f,0.25f,0.1f]},{id:"Pig"},{id:"Skeleton",Equipment:[{id:"bow"}]}]}"#),
        r#"{Entities:[{ArmorDropChances:[0.25f,0.1f,0.0f,0.0f],ArmorItems:[{Count:1b,id:"minecraft:iron_boots"},{},{},{}],Equipment:[{Count:1b,id:"stone"},{Count:1b,id:"minecraft:iron_boots"}],HandDropChances:[0.5f,0.0f],HandItems:[{Count:1b,id:"minecraft:stone"},{}],id:"Zombie"},{id:"Pig"},{Equipment:[{id:"bow"}],HandItems:[{id:"minecraft:bow"},{}],id:"Skeleton"}]}"#
    );
    assert_eq!(update("player", 99, 100, r#"{Inventory:[{id:"stone",Count:2b}]}"#), r#"{Inventory:[{Count:2b,id:"minecraft:stone"}]}"#);
    assert_eq!(
        update("entity_chunk", 99, 100, r#"{Entities:[{id:"Item",Item:{id:"chest",tag:{BlockEntityTag:{Items:[{id:"dirt"}]}}}}]}"#),
        r#"{Entities:[{Item:{id:"minecraft:chest",tag:{BlockEntityTag:{Items:[{id:"minecraft:dirt"}],id:"Chest"}}},id:"Item"}]}"#
    );
}

#[test]
#[ignore]
fn schemas_match_the_vanilla_dump() {
    let path = std::env::var("WAFER_SCHEMA_DUMP").expect("WAFER_SCHEMA_DUMP points at the SchemaDumpOracle output");
    let expected = std::fs::read_to_string(path).unwrap();
    let max: i32 = std::env::var("WAFER_SCHEMA_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(i32::MAX);
    let fixer = get_data_fixer();
    let mut mismatches = 0;
    for line in expected.lines() {
        let (key, rest) = line.split_once(' ').unwrap();
        let (name, canonical) = rest.split_once(" = ").unwrap();
        let key: i32 = key.parse().unwrap();
        if key > max {
            break;
        }
        let schema = fixer.get_schema(key);
        assert_eq!(schema.version_key, key, "schema {key} is missing");
        let actual = crate::datafixers::types::canonical(&schema.get_type(TypeReference(Box::leak(name.to_string().into_boxed_str()))));
        if actual != canonical {
            mismatches += 1;
            if mismatches <= 5 {
                eprintln!("{key} {name}\n  vanilla: {canonical}\n  wafer:   {actual}");
            }
        }
    }
    assert_eq!(mismatches, 0);
}

#[test]
#[ignore]
fn updates_match_the_vanilla_oracle() {
    let cases = std::fs::read_to_string(std::env::var("WAFER_DFU_CASES").expect("WAFER_DFU_CASES points at the DfuOracle input")).unwrap();
    let expected = std::fs::read_to_string(std::env::var("WAFER_DFU_EXPECTED").expect("WAFER_DFU_EXPECTED points at the DfuOracle output")).unwrap();
    let mut expected = expected.lines();
    let mut mismatches = 0;
    for case in cases.lines().filter(|line| !line.is_empty()) {
        let fields: Vec<&str> = case.splitn(4, '\t').collect();
        let ty: &'static str = Box::leak(fields[0].to_string().into_boxed_str());
        let actual = update(ty, fields[1].parse().unwrap(), fields[2].parse().unwrap(), fields[3]);
        let vanilla = expected.next().expect("an expected line");
        if actual != vanilla {
            mismatches += 1;
            eprintln!("{case}\n  vanilla: {vanilla}\n  wafer:   {actual}");
        }
    }
    assert_eq!(mismatches, 0);
}

#[test]
fn schema_hooks_match_vanilla() {
    let cases: [(&str, &str, &str); 44] = [
        ("unpack", r#"{CriteriaName:"minecraft.mined:minecraft.stone",DisplayName:"x"}"#, r#"{CriteriaName:"minecraft.mined:minecraft.stone",CriteriaType:{id:"minecraft:stone",type:"minecraft:mined"},DisplayName:"x"}"#),
        ("unpack", r#"{CriteriaName:"dummy"}"#, r#"{CriteriaName:"dummy",CriteriaType:{id:"dummy",type:"_special"}}"#),
        ("unpack", r#"{CriteriaName:"a:b:c"}"#, r#"{CriteriaName:"a:b:c",CriteriaType:{id:"a:b:c",type:"_special"}}"#),
        ("unpack", r#"{CriteriaName:"minecraft.custom:minecraft.jump"}"#, r#"{CriteriaName:"minecraft.custom:minecraft.jump",CriteriaType:{id:"minecraft:jump",type:"minecraft:custom"}}"#),
        ("unpack", r#"{CriteriaName:"Bad Space:x"}"#, r#"{CriteriaName:"Bad Space:x",CriteriaType:{id:"Bad Space:x",type:"_special"}}"#),
        ("unpack", r#"{CriteriaName:"stat.mineBlock.minecraft.stone"}"#, r#"{CriteriaName:"stat.mineBlock.minecraft.stone",CriteriaType:{id:"stat.mineBlock.minecraft.stone",type:"_special"}}"#),
        ("unpack", r#"{CriteriaName:"x:"}"#, r#"{CriteriaName:"x:",CriteriaType:{id:"minecraft:",type:"minecraft:x"}}"#),
        ("unpack", r#"{CriteriaName:":y"}"#, r#"{CriteriaName:":y",CriteriaType:{id:"minecraft:y",type:"minecraft:"}}"#),
        ("unpack", r#"{DisplayName:"x"}"#, r#"{DisplayName:"x"}"#),
        ("unpack", r#"{CriteriaName:5}"#, r#"{CriteriaName:5}"#),
        ("repack", r#"{CriteriaType:{type:"minecraft:mined",id:"minecraft:stone"},DisplayName:"x"}"#, r#"{CriteriaName:"minecraft.mined:minecraft.stone",DisplayName:"x"}"#),
        ("repack", r#"{CriteriaType:{type:"_special",id:"dummy"}}"#, r#"{CriteriaName:"dummy"}"#),
        ("repack", r#"{CriteriaType:{type:"a"}}"#, r#"{CriteriaType:{type:"a"}}"#),
        ("repack", r#"{CriteriaType:{type:"minecraft:custom",id:"minecraft:jump"}}"#, r#"{CriteriaName:"minecraft.custom:minecraft.jump"}"#),
        ("repack", r#"{CriteriaType:{type:"Bad Space",id:"x"}}"#, r#"{CriteriaName:"Bad Space:minecraft.x"}"#),
        ("repack", r#"{CriteriaType:5}"#, r#"{CriteriaType:5}"#),
        ("repack", r#"{DisplayName:"x"}"#, r#"{DisplayName:"x"}"#),
        ("plain", r#""minecraft:stone""#, r#"{count:1,id:"minecraft:stone"}"#),
        ("plain", r#"{id:"x"}"#, r#"{id:"x"}"#),
        ("plain", r#"5"#, r#"5"#),
        ("v704", r#"{id:"minecraft:chest",tag:{BlockEntityTag:{Items:[]}}}"#, r#"{id:"minecraft:chest",tag:{BlockEntityTag:{Items:[],id:"minecraft:chest"}}}"#),
        ("v704", r#"{id:"minecraft:oak_sign",tag:{BlockEntityTag:{}}}"#, r#"{id:"minecraft:oak_sign",tag:{BlockEntityTag:{id:"minecraft:sign"}}}"#),
        ("v704", r#"{id:"minecraft:white_shulker_box",tag:{BlockEntityTag:{x:1}}}"#, r#"{id:"minecraft:white_shulker_box",tag:{BlockEntityTag:{id:"minecraft:shulker_box",x:1}}}"#),
        ("v704", r#"{id:"minecraft:pig_spawn_egg",tag:{EntityTag:{}}}"#, r#"{id:"minecraft:pig_spawn_egg",tag:{EntityTag:{}}}"#),
        ("v704", r#"{id:"minecraft:armor_stand",tag:{EntityTag:{}}}"#, r#"{id:"minecraft:armor_stand",tag:{EntityTag:{id:"ArmorStand"}}}"#),
        ("v704", r#"{id:"minecraft:boat",tag:{EntityTag:{}}}"#, r#"{id:"minecraft:boat",tag:{EntityTag:{}}}"#),
        ("v704", r#"{id:"minecraft:painting",tag:{EntityTag:{id:"keep"}}}"#, r#"{id:"minecraft:painting",tag:{EntityTag:{id:"keep"}}}"#),
        ("v704", r#"{id:"minecraft:stone",tag:{BlockEntityTag:{}}}"#, r#"{id:"minecraft:stone",tag:{BlockEntityTag:{}}}"#),
        ("v704", r#"{tag:{BlockEntityTag:{}}}"#, r#"{tag:{BlockEntityTag:{}}}"#),
        ("v704", r#"{id:"minecraft:enhanting_table",tag:{BlockEntityTag:{}}}"#, r#"{id:"minecraft:enhanting_table",tag:{BlockEntityTag:{id:"minecraft:enchanting_table"}}}"#),
        ("v704", r#"{id:"minecraft:spawner",tag:{BlockEntityTag:{}}}"#, r#"{id:"minecraft:spawner",tag:{BlockEntityTag:{id:"minecraft:mob_spawner"}}}"#),
        ("v704", r#"{id:"minecraft:stone"}"#, r#"{id:"minecraft:stone"}"#),
        ("v705", r#"{id:"minecraft:chest",tag:{BlockEntityTag:{Items:[]}}}"#, r#"{id:"minecraft:chest",tag:{BlockEntityTag:{Items:[],id:"minecraft:chest"}}}"#),
        ("v705", r#"{id:"minecraft:oak_sign",tag:{BlockEntityTag:{}}}"#, r#"{id:"minecraft:oak_sign",tag:{BlockEntityTag:{id:"minecraft:sign"}}}"#),
        ("v705", r#"{id:"minecraft:white_shulker_box",tag:{BlockEntityTag:{x:1}}}"#, r#"{id:"minecraft:white_shulker_box",tag:{BlockEntityTag:{id:"minecraft:shulker_box",x:1}}}"#),
        ("v705", r#"{id:"minecraft:pig_spawn_egg",tag:{EntityTag:{}}}"#, r#"{id:"minecraft:pig_spawn_egg",tag:{EntityTag:{id:"minecraft:pig"}}}"#),
        ("v705", r#"{id:"minecraft:armor_stand",tag:{EntityTag:{}}}"#, r#"{id:"minecraft:armor_stand",tag:{EntityTag:{id:"minecraft:armor_stand"}}}"#),
        ("v705", r#"{id:"minecraft:boat",tag:{EntityTag:{}}}"#, r#"{id:"minecraft:boat",tag:{EntityTag:{id:"minecraft:oak_boat"}}}"#),
        ("v705", r#"{id:"minecraft:painting",tag:{EntityTag:{id:"keep"}}}"#, r#"{id:"minecraft:painting",tag:{EntityTag:{id:"keep"}}}"#),
        ("v705", r#"{id:"minecraft:stone",tag:{BlockEntityTag:{}}}"#, r#"{id:"minecraft:stone",tag:{BlockEntityTag:{}}}"#),
        ("v705", r#"{tag:{BlockEntityTag:{}}}"#, r#"{tag:{BlockEntityTag:{}}}"#),
        ("v705", r#"{id:"minecraft:enhanting_table",tag:{BlockEntityTag:{}}}"#, r#"{id:"minecraft:enhanting_table",tag:{BlockEntityTag:{id:"minecraft:enchanting_table"}}}"#),
        ("v705", r#"{id:"minecraft:spawner",tag:{BlockEntityTag:{}}}"#, r#"{id:"minecraft:spawner",tag:{BlockEntityTag:{id:"minecraft:mob_spawner"}}}"#),
        ("v705", r#"{id:"minecraft:stone"}"#, r#"{id:"minecraft:stone"}"#),
    ];
    for (label, input, expected) in cases {
        let hook: fn(Dynamic) -> Dynamic = match label {
            "unpack" => v1451_6::unpack_objective_id,
            "repack" => v1451_6::repack_objective_id,
            "plain" => v3818_5::unpack_plain_id,
            "v704" => v704::add_names_hook,
            "v705" => v705::add_names_hook,
            _ => unreachable!(),
        };
        let value = crate::nbt::parse_fully(input).expect("valid snbt");
        let output = hook(Dynamic::new(value));
        assert_eq!(crate::nbt::to_string(output.get_value().expect("a value")), expected, "{label} {input}");
    }
}


const CASES_FROM_4531: &[(&str, i32, i32, &str, &str)] = &[
    ("entity_chunk", 4530, 4531, r#"{Entities:[{id:"minecraft:copper_golem",weather_state:1},{id:"minecraft:pig"}]}"#, r#"{Entities:[{id:"minecraft:copper_golem",weather_state:1},{id:"minecraft:pig"}]}"#),
    ("entity_chunk", 4534, 4535, r#"{Entities:[{id:"minecraft:copper_golem",weather_state:2},{id:"minecraft:copper_golem",weather_state:0},{id:"minecraft:copper_golem",weather_state:3},{id:"minecraft:copper_golem"},{id:"minecraft:cow",weather_state:2}]}"#, r#"{Entities:[{id:"minecraft:copper_golem",weather_state:"weathered"},{id:"minecraft:copper_golem",weather_state:"unaffected"},{id:"minecraft:copper_golem",weather_state:"oxidized"},{id:"minecraft:copper_golem"},{id:"minecraft:cow",weather_state:2}]}"#),
    ("chunk", 4536, 4537, r#"{isLightOn:1b,Status:"minecraft:full",sections:[{Y:0b,BlockLight:[B;1,2],SkyLight:[B;3],block_states:{palette:[{Name:"minecraft:stone"}]}},{Y:1b}],xPos:0,zPos:0}"#, r#"{Status:"minecraft:full",sections:[{Y:0b,block_states:{palette:[{Name:"minecraft:stone"}]}},{Y:1b}],xPos:0,zPos:0}"#),
    ("chunk", 4540, 4541, r#"{Status:"minecraft:full",sections:[{Y:0b,block_states:{palette:[{Name:"minecraft:chain",Properties:{axis:"y"}},{Name:"chain"},{Name:"minecraft:stone"}]}}],block_entities:[]}"#, r#"{Status:"minecraft:full",block_entities:[],sections:[{Y:0b,block_states:{palette:[{Name:"minecraft:iron_chain",Properties:{axis:"y"}},{Name:"minecraft:iron_chain"},{Name:"minecraft:stone"}]}}]}"#),
    ("player", 4540, 4541, r#"{Inventory:[{id:"minecraft:chain",count:2,Slot:0b},{id:"chain",count:1,Slot:1b},{id:"minecraft:stone",count:1,Slot:2b}]}"#, r#"{Inventory:[{Slot:0b,count:2,id:"minecraft:iron_chain"},{Slot:1b,count:1,id:"minecraft:iron_chain"},{Slot:2b,count:1,id:"minecraft:stone"}]}"#),
    ("level", 4543, 4544, r#"{BorderCenterX:1.5d,BorderCenterZ:-2.0d,BorderSize:100.0d,BorderSizeLerpTime:5L,BorderSizeLerpTarget:50.0d,BorderSafeZone:4.0d,BorderDamagePerBlock:0.3d,BorderWarningBlocks:7,BorderWarningTime:20,LevelName:"x"}"#, r#"{LevelName:"x",world_border:{center_x:1.5d,center_z:-2.0d,damage_per_block:0.3d,lerp_target:50.0d,lerp_time:5L,safe_zone:4.0d,size:100.0d,warning_blocks:7,warning_time:20}}"#),
    ("level", 4543, 4544, r#"{LevelName:"x"}"#, r#"{LevelName:"x",world_border:{center_x:0.0d,center_z:0.0d,damage_per_block:0.2d,lerp_target:0.0d,lerp_time:0L,safe_zone:5.0d,size:5.9999968E7d,warning_blocks:5,warning_time:15}}"#),
    ("level", 4547, 4548, r#"{SpawnX:1,SpawnY:64,SpawnZ:-3,SpawnAngle:90.0f,LevelName:"x"}"#, r#"{LevelName:"x",spawn:{dimension:"minecraft:overworld",pitch:0.0f,pos:[I;1,64,-3],yaw:90.0f}}"#),
    ("player", 4547, 4548, r#"{respawn:{pos:[I;1,2,3],angle:45.0f,dimension:"minecraft:the_nether",forced:1b}}"#, r#"{respawn:{dimension:"minecraft:the_nether",forced:1b,pitch:0.0f,pos:[I;1,2,3],yaw:45.0f}}"#),
    ("player", 4547, 4548, r#"{respawn:{pos:[I;1,2,3]}}"#, r#"{respawn:{dimension:"minecraft:overworld",pitch:0.0f,pos:[I;1,2,3],yaw:0.0f}}"#),
    ("entity_chunk", 4647, 4648, r#"{Entities:[{id:"minecraft:nautilus"},{id:"minecraft:zombie_nautilus"}]}"#, r#"{Entities:[{id:"minecraft:nautilus"},{id:"minecraft:zombie_nautilus"}]}"#),
    ("player", 4648, 4649, r#"{Inventory:[{id:"minecraft:trident",count:1,Slot:0b,components:{"minecraft:consumable":{animation:"spear",consume_seconds:1.0f}}},{id:"minecraft:apple",count:1,Slot:1b,components:{"minecraft:consumable":{animation:"eat"}}}]}"#, r#"{Inventory:[{Slot:0b,components:{"minecraft:consumable":{animation:"trident",consume_seconds:1.0f}},count:1,id:"minecraft:trident"},{Slot:1b,components:{"minecraft:consumable":{animation:"eat"}},count:1,id:"minecraft:apple"}]}"#),
    ("debug_profile", 4649, 4650, r#"{custom:{"minecraft:fps":"inF3","minecraft:tps":"always","minecraft:x":5}}"#, r#"{custom:{"minecraft:fps":"inOverlay","minecraft:tps":"always","minecraft:x":5}}"#),
    ("options", 4650, 4651, r#"{graphicsMode:"2",version:4650}"#, r#"{cutoutLeaves:"true",graphicsMode:"2",graphicsPreset:"custom",improvedTransparency:"true",version:4650,vignette:"true",weatherRadius:"10"}"#),
    ("options", 4650, 4651, r#"{graphicsMode:"0"}"#, r#"{cutoutLeaves:"false",graphicsMode:"0",graphicsPreset:"custom",improvedTransparency:"false",vignette:"false",weatherRadius:"5"}"#),
    ("options", 4650, 4651, r#"{fov:"0.5"}"#, r#"{cutoutLeaves:"true",fov:"0.5",graphicsPreset:"custom",improvedTransparency:"false",vignette:"true",weatherRadius:"10"}"#),
    ("entity_chunk", 4655, 4656, r#"{Entities:[{id:"minecraft:parched"},{id:"minecraft:camel_husk"}]}"#, r#"{Entities:[{id:"minecraft:parched"},{id:"minecraft:camel_husk"}]}"#),
    ("saved_data/world_border", 4656, 4657, r#"{data:{warning_time:10,size:5.0d},DataVersion:4656}"#, r#"{DataVersion:4656,data:{size:5.0d,warning_time:200}}"#),
    ("saved_data/world_border", 4656, 4657, r#"{data:{size:5.0d}}"#, r#"{data:{size:5.0d}}"#),
    ("level", 4657, 4658, r#"{GameRules:{doFireTick:"false",keepInventory:"true",randomTickSpeed:"5",snowAccumulationHeight:"20",disableRaids:"true",spawnChunkRadius:"2",unknownRule:"x",commandModificationBlockLimit:"-5",minecartMaxSpeed:"abc"},LevelName:"x"}"#, r#"{LevelName:"x",game_rules:{"minecraft:fire_spread_radius_around_player":0,"minecraft:keep_inventory":1b,"minecraft:max_block_modifications":1,"minecraft:max_minecart_speed":"abc","minecraft:max_snow_accumulation_height":8,"minecraft:raids":0b,"minecraft:random_tick_speed":5,unknownRule:"x"}}"#),
    ("level", 4657, 4658, r#"{GameRules:{doFireTick:"true",allowFireTicksAwayFromPlayer:"true"},LevelName:"x"}"#, r#"{LevelName:"x",game_rules:{"minecraft:fire_spread_radius_around_player":-1}}"#),
    ("level", 4657, 4658, r#"{GameRules:{doFireTick:"TRUE"},LevelName:"x"}"#, r#"{LevelName:"x",game_rules:{}}"#),
    ("options", 4660, 4661, r#"{showNowPlayingToast:"true"}"#, r#"{musicToast:"pause_and_toast"}"#),
    ("options", 4660, 4661, r#"{showNowPlayingToast:"false"}"#, r#"{musicToast:"never"}"#),
    ("debug_profile", 4762, 4763, r#"{custom:{"minecraft:looking_at_block":"inOverlay","minecraft:looking_at_fluid":"always","minecraft:looking_at_block_state":"never"}}"#, r#"{custom:{"minecraft:looking_at_block_state":"never","minecraft:looking_at_fluid_state":"always"}}"#),
    ("entity_chunk", 4762, 4763, r#"{Entities:[{id:"minecraft:villager"},{id:"minecraft:zombie_villager",VillagerDataFinalized:0b},{id:"minecraft:pig"}]}"#, r#"{Entities:[{VillagerDataFinalized:1b,id:"minecraft:villager"},{VillagerDataFinalized:1b,id:"minecraft:zombie_villager"},{id:"minecraft:pig"}]}"#),
    ("level", 4765, 4766, r#"{Time:100L,DayTime:5000L,LevelName:"x"}"#, r#"{LevelName:"x",Time:100L,world_clocks:{"minecraft:overworld":{paused:0b,total_ticks:5000L}}}"#),
    ("level", 4765, 4766, r#"{Time:100L,LevelName:"x"}"#, r#"{LevelName:"x",Time:100L,world_clocks:{"minecraft:overworld":{paused:0b,total_ticks:100L}}}"#),
    ("advancements", 4768, 4769, r#"{"minecraft:recipes/building_blocks/quartz_slab_from_stonecutting":{done:1b,criteria:{a:"2020"}},"minecraft:story/root":{done:1b},DataVersion:4768}"#, r#"{DataVersion:4768,"minecraft:recipes/building_blocks/quartz_slab_from_quartz_block_stonecutting":{criteria:{a:"2020"},done:1b},"minecraft:story/root":{done:1b}}"#),
    ("player", 4768, 4769, r#"{recipeBook:{recipes:["minecraft:quartz_slab_from_stonecutting","minecraft:stone"],toBeDisplayed:["quartz_slab_from_stonecutting"]}}"#, r#"{recipeBook:{recipes:["minecraft:quartz_slab_from_quartz_block_stonecutting","minecraft:stone"],toBeDisplayed:["minecraft:quartz_slab_from_quartz_block_stonecutting"]}}"#),
    ("level", 4770, 4771, r#"{Difficulty:3b,hardcore:1b,DifficultyLocked:0b,LevelName:"x"}"#, r#"{LevelName:"x",difficulty_settings:{difficulty:"hard",hardcore:1b,locked:0b},scheduled_events:{events:[]},wandering_trader_migration_data:{spawn_chance:25,spawn_delay:24000},weather_data:{clear_weather_time:0,rain_time:0,raining:0b,thunder_time:0,thundering:0b}}"#),
    ("level", 4770, 4771, r#"{LevelName:"x"}"#, r#"{LevelName:"x",difficulty_settings:{difficulty:"normal",hardcore:0b,locked:0b},scheduled_events:{events:[]},wandering_trader_migration_data:{spawn_chance:25,spawn_delay:24000},weather_data:{clear_weather_time:0,rain_time:0,raining:0b,thunder_time:0,thundering:0b}}"#),
    ("level", 4770, 4771, r#"{DragonFight:{NeedsStateScanning:0b,DragonKilled:1b,PreviouslyKilled:1b,IsRespawning:1b,Gateways:[I;1,2]},WanderingTraderSpawnDelay:100,WanderingTraderSpawnChance:50,WanderingTraderId:[I;1,2,3,4],clearWeatherTime:5,rainTime:6,thunderTime:7,raining:1b,thundering:0b,ScheduledEvents:[{Name:"e",TriggerTime:5L,Callback:{Type:"function",Name:"f:g"}}],WorldGenSettings:{generate_features:1b,seed:5L,dimensions:{}},LevelName:"x"}"#, r#"{LevelName:"x",difficulty_settings:{difficulty:"normal",hardcore:0b,locked:0b},dragon_fight:{dragon_killed:1b,gateways:[I;1,2],needs_state_scanning:0b,previously_killed:1b,respawn_stage:"start",respawn_time:0},scheduled_events:{events:[{callback:{id:"f:g",type:"function"},id:"e",trigger_time:5L}]},wandering_trader_migration_data:{spawn_chance:50,spawn_delay:100},weather_data:{clear_weather_time:5,rain_time:6,raining:1b,thunder_time:7,thundering:0b},world_gen_settings:{dimensions:{},generate_structures:1b,seed:5L}}"#),
    ("entity_chunk", 4880, 4881, r#"{Entities:[{id:"minecraft:sulfur_cube"}]}"#, r#"{Entities:[{id:"minecraft:sulfur_cube"}]}"#),
    ("chunk", 4884, 4885, r#"{Status:"minecraft:full",block_entities:[{id:"minecraft:bed",x:0,y:0,z:0},{id:"minecraft:chest",x:1,y:0,z:0,Items:[{id:"minecraft:red_bed",count:1,Slot:0b,components:{"minecraft:block_entity_data":{id:"minecraft:bed"}}}]}],sections:[]}"#, r#"{Status:"minecraft:full",block_entities:[{Items:[{Slot:0b,components:{},count:1,id:"minecraft:red_bed"}],id:"minecraft:chest",x:1,y:0,z:0}],sections:[]}"#),
    ("player", 4884, 4885, r#"{Inventory:[{id:"minecraft:red_bed",count:1,Slot:0b,components:{"minecraft:block_entity_data":{id:"minecraft:bed",x:0}}},{id:"minecraft:chest",count:1,Slot:1b,components:{"minecraft:block_entity_data":{id:"minecraft:chest"}}}]}"#, r#"{Inventory:[{Slot:0b,components:{},count:1,id:"minecraft:red_bed"},{Slot:1b,components:{"minecraft:block_entity_data":{id:"minecraft:chest"}},count:1,id:"minecraft:chest"}]}"#),
    ("entity_chunk", 4884, 4885, r#"{Entities:[{id:"minecraft:falling_block",BlockState:{Name:"minecraft:red_bed"},TileEntityData:{id:"minecraft:bed"}},{id:"minecraft:falling_block",TileEntityData:{id:"minecraft:chest"}}]}"#, r#"{Entities:[{BlockState:{Name:"minecraft:red_bed"},id:"minecraft:falling_block"},{TileEntityData:{id:"minecraft:chest"},id:"minecraft:falling_block"}]}"#),
    ("structure", 4884, 4885, r#"{blocks:[{pos:[0,0,0],state:0,nbt:{id:"minecraft:bed"}},{pos:[1,0,0],state:1,nbt:{id:"minecraft:chest"}}],palette:[{Name:"minecraft:red_bed"},{Name:"minecraft:chest"}],entities:[],size:[2,1,1]}"#, r#"{blocks:[{pos:[0,0,0],state:0},{nbt:{id:"minecraft:chest"},pos:[1,0,0],state:1}],entities:[],palette:[{Name:"minecraft:red_bed"},{Name:"minecraft:chest"}],size:[2,1,1]}"#),
    ("entity_chunk", 4887, 4888, r#"{Entities:[{id:"minecraft:zombie",attributes:[{id:"minecraft:nameplate_distance",base:1.0d},{id:"minecraft:max_health",base:20.0d}]}]}"#, r#"{Entities:[{attributes:[{base:1.0d,id:"minecraft:name_tag_distance"},{base:20.0d,id:"minecraft:max_health"}],id:"minecraft:zombie"}]}"#),
    ("player", 4887, 4888, r#"{attributes:[{id:"minecraft:nameplate_distance",base:1.0d}],Inventory:[{id:"minecraft:stick",count:1,Slot:0b,components:{"minecraft:attribute_modifiers":[{type:"minecraft:nameplate_distance",amount:1.0d,id:"a",operation:"add_value"}]}}]}"#, r#"{Inventory:[{Slot:0b,components:{"minecraft:attribute_modifiers":[{amount:1.0d,id:"a",operation:"add_value",type:"minecraft:name_tag_distance"}]},count:1,id:"minecraft:stick"}],attributes:[{base:1.0d,id:"minecraft:name_tag_distance"}]}"#),
    ("options", 4891, 4892, r#"{fov:"0.5"}"#, r#"{fov:"0.5",preferredGraphicsBackend:"default"}"#),
    ("player", 4995, 4996, r#"{Inventory:[{id:"minecraft:decorated_pot",count:1,Slot:0b,components:{"minecraft:pot_decorations":["minecraft:angler_pottery_sherd","minecraft:brick"]}}]}"#, r#"{Inventory:[{Slot:0b,components:{"minecraft:pot_decorations":{back:{id:"minecraft:angler_pottery_sherd"},front:{id:"minecraft:brick"},left:{id:"minecraft:brick"},right:{id:"minecraft:brick"}}},count:1,id:"minecraft:decorated_pot"}]}"#),
    ("chunk", 4995, 4996, r#"{Status:"minecraft:full",block_entities:[{id:"minecraft:decorated_pot",x:0,y:0,z:0,sherds:["minecraft:angler_pottery_sherd"]},{id:"minecraft:decorated_pot",x:1,y:0,z:0}],sections:[]}"#, r#"{Status:"minecraft:full",block_entities:[{id:"minecraft:decorated_pot",sherds:{back:{id:"minecraft:angler_pottery_sherd"},front:{id:"minecraft:brick"},left:{id:"minecraft:brick"},right:{id:"minecraft:brick"}},x:0,y:0,z:0},{id:"minecraft:decorated_pot",sherds:{back:{id:"minecraft:brick"},front:{id:"minecraft:brick"},left:{id:"minecraft:brick"},right:{id:"minecraft:brick"}},x:1,y:0,z:0}],sections:[]}"#),
    ("chunk", 4996, 4997, r#"{Status:"minecraft:full",__context:{dimension:"minecraft:overworld"},blending_data:{min_section:0},sections:[]}"#, r#"{Status:"minecraft:full",__context:{dimension:"minecraft:overworld"},blending_data:{max_section:20,min_section:-4},sections:[]}"#),
    ("chunk", 4996, 4997, r#"{Status:"minecraft:biomes",__context:{dimension:"minecraft:overworld"},sections:[]}"#, r#"{Status:"minecraft:biomes",__context:{dimension:"minecraft:overworld"},sections:[]}"#),
    ("chunk", 4996, 4997, r#"{Status:"minecraft:biomes",below_zero_retrogen:{target_status:"features"},__context:{dimension:"minecraft:overworld"},sections:[]}"#, r#"{Status:"minecraft:biomes",__context:{dimension:"minecraft:overworld"},below_zero_retrogen:{target_status:"features"},blending_data:{max_section:16,min_section:0},sections:[]}"#),
    ("chunk", 4996, 4997, r#"{Status:"minecraft:full",__context:{dimension:"minecraft:the_nether"},sections:[]}"#, r#"{Status:"minecraft:full",__context:{dimension:"minecraft:the_nether"},sections:[]}"#),
    ("entity_chunk", 4999, 5000, r#"{Entities:[{id:"minecraft:cushion"},{id:"minecraft:poplar_boat"}]}"#, r#"{Entities:[{id:"minecraft:cushion"},{id:"minecraft:poplar_boat"}]}"#),
    ("debug_profile", 5000, 5001, r#"{custom:{"minecraft:post_effect":"always"}}"#, r#"{custom:{"minecraft:post_effects":"always"}}"#),
    ("chunk", 5001, 5002, r#"{Status:"minecraft:full",block_entities:[{id:"minecraft:sign",x:0,y:0,z:0},{id:"minecraft:hanging_sign",x:1,y:0,z:0},{id:"minecraft:chest",x:2,y:0,z:0}],sections:[]}"#, r#"{Status:"minecraft:full",block_entities:[{allow_op_features:1b,id:"minecraft:sign",x:0,y:0,z:0},{allow_op_features:1b,id:"minecraft:hanging_sign",x:1,y:0,z:0},{id:"minecraft:chest",x:2,y:0,z:0}],sections:[]}"#),
    ("chunk", 5005, 5006, r#"{Status:"minecraft:full",sections:[{Y:0b,block_states:{palette:[{Name:"minecraft:oak_log",Properties:{axis:"y"}}]}}],block_entities:[]}"#, r#"{Status:"minecraft:full",block_entities:[],sections:[{Y:0b,block_states:{palette:[{id:"minecraft:oak_log",properties:{axis:"y"}}]}}]}"#),
    ("structure", 5005, 5006, r#"{palette:[{Name:"minecraft:stone"}],blocks:[],entities:[],size:[1,1,1]}"#, r#"{blocks:[],entities:[],palette:[{id:"minecraft:stone"}],size:[1,1,1]}"#),
    ("player", 5006, 5007, r#"{Inventory:[{id:"minecraft:stick",count:1,Slot:0b,components:{"minecraft:swing_animation":{type:"whack",duration:6}}}]}"#, r#"{Inventory:[{Slot:0b,components:{"minecraft:attack_animation":{duration:6,type:"whack"},"minecraft:interact_animation":{duration:6,type:"whack"}},count:1,id:"minecraft:stick"}]}"#),
    ("player", 5007, 5008, r#"{Inventory:[{id:"minecraft:filled_map",count:1,Slot:0b,components:{"minecraft:map_decorations":{"+":{type:"minecraft:mansion",x:1.0d,z:2.0d,rotation:180.0f}},"minecraft:item_name":{translate:"filled_map.mansion"},"minecraft:map_color":4},"minecraft:map_id":5},{id:"minecraft:filled_map",count:1,Slot:1b,components:{"minecraft:map_decorations":{"+":{type:"red_x"}},"minecraft:item_name":{translate:"other"}}},{id:"minecraft:filled_map",count:1,Slot:2b,components:{"minecraft:map_color":5}}]}"#, r#"{Inventory:[{Slot:0b,components:{"minecraft:map_decorations":{"+":{rotation:180.0f,type:"minecraft:mansion",x:1.0d,z:2.0d}}},count:1,id:"minecraft:woodland_explorer_map","minecraft:map_id":5},{Slot:1b,components:{"minecraft:item_name":{translate:"other"},"minecraft:map_decorations":{"+":{type:"red_x"}}},count:1,id:"minecraft:buried_treasure_map"},{Slot:2b,components:{},count:1,id:"minecraft:filled_map"}]}"#),
    ("chunk", 5008, 5009, r#"{Status:"minecraft:full",blending_data:{min_section:-4,max_section:20,heights:[1.0d,1.7976931348623157E308d,-5.5d]},sections:[]}"#, r#"{Status:"minecraft:full",blending_data:{heights:[1.0f,3.4028235E38f,-5.5f],max_section:20,min_section:-4},sections:[]}"#),
    ("player", 5011, 5012, r#"{Inventory:[{id:"minecraft:ocean_explorer_map",count:1,Slot:0b},{id:"jungle_explorer_map",count:1,Slot:1b}]}"#, r#"{Inventory:[{Slot:0b,count:1,id:"minecraft:ocean_monument_map"},{Slot:1b,count:1,id:"minecraft:jungle_pyramid_map"}]}"#),
    ("chunk", 5012, 5013, r#"{Status:"minecraft:noise",Heightmaps:{a:[L;1L]},blending_data:{min_section:0},sections:[{Y:-1b,biomes:{palette:["minecraft:plains"]},block_states:{palette:[{Name:"minecraft:stone"}]}},{Y:0b,block_states:{palette:[{Name:"minecraft:stone"}]}}]}"#, r#"{Status:"minecraft:biomes",sections:[{Y:-1b,biomes:{palette:["minecraft:plains"]}},{Y:0b}]}"#),
    ("chunk", 5012, 5013, r#"{Status:"minecraft:carvers",sections:[{Y:0b,block_states:{palette:[{Name:"minecraft:stone"}]}}]}"#, r#"{Status:"minecraft:terrain",sections:[{Y:0b,block_states:{palette:[{Name:"minecraft:stone"}]}}]}"#),
    ("chunk", 5012, 5013, r#"{Status:"minecraft:full",below_zero_retrogen:{target_status:"minecraft:surface"},sections:[{Y:-1b,block_states:{palette:[{Name:"minecraft:stone"}]}},{Y:0b,block_states:{palette:[{Name:"minecraft:stone"}]}}]}"#, r#"{Status:"minecraft:full",below_zero_retrogen:{target_status:"minecraft:biomes"},sections:[{Y:-1b},{Y:0b}]}"#),
    ("chunk", 5012, 5013, r#"{Status:"minecraft:full",below_zero_retrogen:{target_status:"minecraft:features"},sections:[{Y:-1b,block_states:{palette:[{Name:"minecraft:stone"}]}}]}"#, r#"{Status:"minecraft:full",below_zero_retrogen:{target_status:"minecraft:features"},sections:[{Y:-1b,block_states:{palette:[{Name:"minecraft:stone"}]}}]}"#),
    ("player", 5013, 5014, r#"{Inventory:[{id:"minecraft:stick",count:1,Slot:0b,components:{"minecraft:block_transformer":{a:1},"minecraft:max_damage":5}}]}"#, r#"{Inventory:[{Slot:0b,components:{"minecraft:max_damage":5},count:1,id:"minecraft:stick"}]}"#),
    ("player", 5015, 5016, r#"{Inventory:[{id:"minecraft:map",count:1,Slot:0b,components:{"minecraft:item_name":{translate:"filled_map.buried_treasure"}}},{id:"minecraft:map",count:1,Slot:1b,components:{"minecraft:item_name":{translate:"filled_map.buried_treasure",color:"red"}}}]}"#, r#"{Inventory:[{Slot:0b,components:{"minecraft:item_name":{translate:"item.minecraft.buried_treasure_map"}},count:1,id:"minecraft:map"},{Slot:1b,components:{"minecraft:item_name":{color:"red",translate:"filled_map.buried_treasure"}},count:1,id:"minecraft:map"}]}"#),
    ("chunk", 4530, 5023, r#"{Status:"minecraft:noise",isLightOn:1b,sections:[{Y:0b,BlockLight:[B;1],block_states:{palette:[{Name:"minecraft:chain"}]}}],block_entities:[{id:"minecraft:sign",x:0,y:0,z:0},{id:"minecraft:bed",x:1,y:0,z:0}],xPos:0,zPos:0}"#, r#"{Status:"minecraft:biomes",block_entities:[{allow_op_features:1b,id:"minecraft:sign",x:0,y:0,z:0}],sections:[{Y:0b}],xPos:0,zPos:0}"#),
    ("level", 4530, 5023, r#"{BorderSize:10.0d,SpawnX:5,Time:10L,DayTime:20L,Difficulty:1b,GameRules:{doFireTick:"false"},LevelName:"x",WorldGenSettings:{generate_features:1b,seed:1L,dimensions:{}}}"#, r#"{LevelName:"x",Time:10L,difficulty_settings:{difficulty:"easy",hardcore:0b,locked:0b},game_rules:{"minecraft:fire_spread_radius_around_player":0},scheduled_events:{events:[]},spawn:{dimension:"minecraft:overworld",pitch:0.0f,pos:[I;5,0,0],yaw:0.0f},wandering_trader_migration_data:{spawn_chance:25,spawn_delay:24000},weather_data:{clear_weather_time:0,rain_time:0,raining:0b,thunder_time:0,thundering:0b},world_border:{center_x:0.0d,center_z:0.0d,damage_per_block:0.2d,lerp_target:0.0d,lerp_time:0L,safe_zone:5.0d,size:10.0d,warning_blocks:5,warning_time:15},world_clocks:{"minecraft:overworld":{paused:0b,total_ticks:20L}},world_gen_settings:{dimensions:{},generate_structures:1b,seed:1L}}"#),
];

#[test]
fn fixes_from_4531_match_vanilla() {
    for &(ty, from, to, input, expected) in CASES_FROM_4531 {
        assert_eq!(update(ty, from, to, input), expected, "{ty} {from} -> {to}: {input}");
    }
}
