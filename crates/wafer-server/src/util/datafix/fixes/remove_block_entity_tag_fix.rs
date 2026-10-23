use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{OpticFinder, Typed, field_finder, find_field, named_choice, type_finder};
use crate::datafixers::types::{Type, const_type};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{BLOCK_ENTITY, CHUNK, ENTITY, ITEM_STACK, STRUCTURE};

pub struct RemoveBlockEntityTagFix {
    base: DataFixBase,
    block_entity_ids_to_drop: Arc<Vec<String>>,
    use_legacy_data_structure: bool,
}

impl RemoveBlockEntityTagFix {
    pub fn new(output_schema: &Arc<Schema>, block_entity_ids_to_drop: &[&str]) -> RemoveBlockEntityTagFix {
        RemoveBlockEntityTagFix::with_legacy(output_schema, false, block_entity_ids_to_drop)
    }

    pub fn with_legacy(output_schema: &Arc<Schema>, use_legacy_data_structure: bool, block_entity_ids_to_drop: &[&str]) -> RemoveBlockEntityTagFix {
        RemoveBlockEntityTagFix {
            base: DataFixBase::new(output_schema, true),
            block_entity_ids_to_drop: Arc::new(block_entity_ids_to_drop.iter().map(|id| id.to_string()).collect()),
            use_legacy_data_structure,
        }
    }

    fn create_item_block_entity_remover(&self, block_entity_id_finder: &OpticFinder, item_tag_or_component_key: &str, item_block_entity_data_key: &str) -> TypeRewriteRule {
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let item_tag_finder = find_field(&item_stack_type, item_tag_or_component_key);
        let item_block_entity_finder = find_field(&item_tag_finder.ty(), item_block_entity_data_key);
        let (ids, id_finder, key) = (self.block_entity_ids_to_drop.clone(), block_entity_id_finder.clone(), item_block_entity_data_key.to_string());
        self.fix_type_everywhere_typed(&format!("ItemRemoveBlockEntityTagFix{}", self.get_output_schema().version_key), item_stack_type, move |input| {
            input.update_typed(&item_tag_finder, None, |tag| remove_block_entity(&ids, tag, &item_block_entity_finder, &id_finder, &key))
        })
    }

    fn create_falling_block_block_entity_remover(&self, block_entity_id_finder: &OpticFinder) -> TypeRewriteRule {
        let entity_type = self.get_input_schema().get_type(ENTITY);
        let falling_block_finder = named_choice("minecraft:falling_block", self.get_input_schema().get_choice_type(ENTITY, "minecraft:falling_block"));
        let falling_block_entity_tag_finder = find_field(&falling_block_finder.ty(), "TileEntityData");
        let (ids, id_finder) = (self.block_entity_ids_to_drop.clone(), block_entity_id_finder.clone());
        self.fix_type_everywhere_typed(&format!("FallingBlockEntityRemoveBlockEntityTagFix{}", self.get_output_schema().version_key), entity_type, move |input| {
            input.update_typed(&falling_block_finder, None, |tag| remove_block_entity(&ids, tag, &falling_block_entity_tag_finder, &id_finder, "TileEntityData"))
        })
    }

    fn create_structure_block_entity_remover(&self, block_entity_id_finder: &OpticFinder) -> TypeRewriteRule {
        let structure_type = self.get_input_schema().get_type(STRUCTURE);
        let blocks_finder = find_field(&structure_type, "blocks");
        let block_type_finder = type_finder(list_element(&blocks_finder.ty()));
        let block_nbt_finder = find_field(&block_type_finder.ty(), "nbt");
        let (ids, id_finder) = (self.block_entity_ids_to_drop.clone(), block_entity_id_finder.clone());
        self.fix_type_everywhere_typed(&format!("StructureRemoveBlockEntityTagFix{}", self.get_output_schema().version_key), structure_type, move |input| {
            input.update_typed(&blocks_finder, None, |tag| tag.update_typed(&block_type_finder, None, |block_tag| remove_block_entity(&ids, block_tag, &block_nbt_finder, &id_finder, "nbt")))
        })
    }

    fn create_chunk_block_entity_remover(&self, block_entity_id_finder: &OpticFinder) -> TypeRewriteRule {
        let chunk_type = self.get_input_schema().get_type(CHUNK);
        let block_entities_finder = find_field(&chunk_type, "block_entities");
        let block_entity_elements_type = list_element(&block_entities_finder.ty());
        let block_entity_type_finder = type_finder(self.get_input_schema().get_type(BLOCK_ENTITY));
        let chunk_type_out = self.get_output_schema().get_type(CHUNK);
        let (ids, id_finder) = (self.block_entity_ids_to_drop.clone(), block_entity_id_finder.clone());
        self.fix_type_everywhere_typed_to(&format!("BlockEntityChunkRemover{}", self.get_output_schema().version_key), chunk_type, chunk_type_out, move |input| {
            input.update(&block_entities_finder, |list_tag| match list_tag {
                Value::List(block_entities) => Value::List(
                    block_entities
                        .into_iter()
                        .filter(|untyped_block_entity| {
                            let typed_block_entity = Typed::new(block_entity_elements_type.clone(), untyped_block_entity.clone());
                            let typed_block_entity_unwrapped = typed_block_entity.get_or_create_typed(&block_entity_type_finder);
                            let block_entity_id = typed_block_entity_unwrapped.get_optional(&id_finder).and_then(|id| id.as_str().map(str::to_string)).unwrap_or_default();
                            !ids.contains(&block_entity_id)
                        })
                        .collect(),
                ),
                other => other,
            })
        })
    }

    fn create_unchecked_converter_hack(&self) -> TypeRewriteRule {
        self.convert_unchecked(
            &format!("ItemRemoveBlockEntityTagFix - update block entity type{}", self.get_output_schema().version_key),
            self.get_input_schema().get_type(BLOCK_ENTITY),
            self.get_output_schema().get_type(BLOCK_ENTITY),
        )
    }
}

impl DataFix for RemoveBlockEntityTagFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let block_entity_id_finder = field_finder("id", const_type(ConstType::NamespacedString));
        if self.use_legacy_data_structure {
            TypeRewriteRule::seq(vec![
                self.create_item_block_entity_remover(&block_entity_id_finder, "tag", "BlockEntityTag"),
                self.create_falling_block_block_entity_remover(&block_entity_id_finder),
                self.create_structure_block_entity_remover(&block_entity_id_finder),
                self.create_unchecked_converter_hack(),
            ])
        } else {
            TypeRewriteRule::seq(vec![
                self.create_item_block_entity_remover(&block_entity_id_finder, "components", "minecraft:block_entity_data"),
                self.create_falling_block_block_entity_remover(&block_entity_id_finder),
                self.create_structure_block_entity_remover(&block_entity_id_finder),
                self.create_chunk_block_entity_remover(&block_entity_id_finder),
                self.create_unchecked_converter_hack(),
            ])
        }
    }
}

fn list_element(ty: &Arc<Type>) -> Arc<Type> {
    match &**ty {
        Type::List(element) => element.clone(),
        other => panic!("Not a list type: {other:?}"),
    }
}

fn remove_block_entity(ids: &[String], tag: Typed, block_entity_finder: &OpticFinder, block_entity_id_finder: &OpticFinder, block_entity_field_name: &str) -> Typed {
    let Some(block_entity) = tag.get_optional_typed(block_entity_finder) else { return tag };
    let block_entity_id = block_entity.get_optional(block_entity_id_finder).and_then(|id| id.as_str().map(str::to_string)).unwrap_or_default();
    if !ids.contains(&block_entity_id) {
        return tag;
    }
    let ty = tag.ty.clone();
    crate::util::write_and_read_typed_or_throw(&tag, &ty, |tag_data| tag_data.remove(block_entity_field_name))
}
