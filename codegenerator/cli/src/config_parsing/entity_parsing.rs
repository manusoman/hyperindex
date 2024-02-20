use super::{
    system_config::{EntityMap, GraphQlEnumMap},
    validation::{
        check_enums_for_internal_reserved_words, check_names_from_schema_for_reserved_words,
        is_valid_postgres_db_name,
    },
};
use crate::{
    capitalization::{Capitalize, CapitalizedOptions},
    utils::unique_hashmap,
};
use anyhow::{anyhow, Context};
use ethers::abi::ethabi::ParamType as EthAbiParamType;
use graphql_parser::schema::{
    Definition, Directive, Document, EnumType, Field as ObjField, ObjectType, Type as ObjType,
    TypeDefinition, Value,
};
use serde::{Serialize, Serializer};
use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Display},
    path::PathBuf,
};
use subenum::subenum;

#[derive(Debug, Clone, PartialEq)]
pub struct Schema {
    pub entities: HashMap<String, Entity>,
    pub enums: HashMap<String, GraphQLEnum>,
}

enum TypeDef<'a> {
    Entity(&'a Entity),
    Enum(&'a GraphQLEnum),
}

impl Schema {
    pub fn empty() -> Self {
        Schema {
            entities: HashMap::new(),
            enums: HashMap::new(),
        }
    }

    pub fn new(entities: Vec<Entity>, enums: Vec<GraphQLEnum>) -> anyhow::Result<Self> {
        let entities = unique_hashmap::from_vec_no_duplicates(
            entities.into_iter().map(|e| (e.name.clone(), e)).collect(),
        )
        .context("Found entities with duplicate names")?;
        let enums = unique_hashmap::from_vec_no_duplicates(
            enums.into_iter().map(|e| (e.name.clone(), e)).collect(),
        )
        .context("Found enums with duplicate names")?;

        Self { entities, enums }.validate()
    }

    fn from_document(document: Document<String>) -> anyhow::Result<Self> {
        let entities = document
            .definitions
            .iter()
            .filter_map(|d| match d {
                Definition::TypeDefinition(type_def) => Some(type_def),
                _ => None,
            })
            .filter_map(|type_def| match type_def {
                TypeDefinition::Object(obj) => Some(obj),
                _ => None,
            })
            .map(|obj| Entity::from_object(obj))
            .collect::<anyhow::Result<Vec<Entity>>>()
            .context("Failed constructing entities in schema from document")?;

        let enums = document
            .definitions
            .iter()
            .filter_map(|d| match d {
                Definition::TypeDefinition(type_def) => Some(type_def),
                _ => None,
            })
            .filter_map(|type_def| match type_def {
                TypeDefinition::Enum(obj) => Some(obj),
                _ => None,
            })
            .map(|obj| GraphQLEnum::from_enum(obj))
            .collect::<anyhow::Result<Vec<GraphQLEnum>>>()
            .context("Failed constructing enums in schema from document")?;

        Self::new(entities, enums)
    }

    pub fn parse_from_file(path_to_schema: &PathBuf) -> anyhow::Result<Self> {
        let schema_string = std::fs::read_to_string(&path_to_schema).context(format!(
            "EE200: Failed to read schema file at {}. Please ensure that the schema file is \
             placed correctly in the directory.",
            &path_to_schema.to_str().unwrap_or_else(|| "bad file path"),
        ))?;

        let schema_doc = graphql_parser::parse_schema::<String>(&schema_string)
            .context("EE201: Failed to parse schema as document")?;

        Self::from_document(schema_doc).context("Failed converting schema doc to schema struct")
    }

    fn validate(self) -> anyhow::Result<Self> {
        self.check_enum_type_defs()?
            .check_schema_for_reserved_words()?
            .check_duplicate_naming_between_enums_and_entities()?
            .check_related_type_defs_exist()?
            .validate_entity_field_types()
    }

    fn get_all_enum_type_names(&self) -> Vec<String> {
        self.enums.keys().cloned().collect()
    }
    fn get_all_enum_values(&self) -> Vec<String> {
        self.enums.values().flat_map(|v| v.values.clone()).collect()
    }
    fn get_all_entity_type_names(&self) -> Vec<String> {
        self.entities.keys().cloned().collect()
    }
    fn get_all_entity_field_names(&self) -> Vec<String> {
        self.entities
            .values()
            .flat_map(|v| v.fields.values())
            .map(|v| v.name.clone())
            .collect()
    }

    fn check_enum_type_defs(self) -> anyhow::Result<Self> {
        match check_enums_for_internal_reserved_words(self.get_all_enum_type_names()) {
            reserved_enum_types_used if reserved_enum_types_used.is_empty() => Ok(self),
            reserved_enum_types_used => Err(anyhow!(
                "EE211: Schema contains the following reserved enum names: {}",
                reserved_enum_types_used.join(", ")
            )),
        }
    }

    fn check_schema_for_reserved_words(self) -> anyhow::Result<Self> {
        let all_names = vec![
            self.get_all_enum_type_names(),
            self.get_all_enum_values(),
            self.get_all_entity_type_names(),
            self.get_all_entity_field_names(),
        ]
        .iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();

        match check_names_from_schema_for_reserved_words(all_names) {
            reserved_enum_types_used if reserved_enum_types_used.is_empty() => Ok(self),
            reserved_enum_types_used => Err(anyhow!(
                "EE210: Schema contains the following reserved keywords: {}",
                reserved_enum_types_used.join(", ")
            )),
        }
    }

    fn check_duplicate_naming_between_enums_and_entities(self) -> anyhow::Result<Self> {
        let duplicate_names = self
            .get_all_enum_type_names()
            .into_iter()
            .filter(|k| self.entities.get(k).is_some())
            .collect::<Vec<_>>();
        if !duplicate_names.is_empty() {
            Err(anyhow!(
                "EE213: Schema contains the following enums and entities with the same name, all \
                 type definitions must be unique in the schema: {}",
                duplicate_names.join(", ")
            ))
        } else {
            Ok(self)
        }
    }

    fn try_get_type_def(&self, name: &String) -> anyhow::Result<TypeDef> {
        match (self.entities.get(name), self.enums.get(name)) {
            (None, None) => Err(anyhow!("No type definition '{}' exists in schema", name)),
            (Some(_), Some(_)) => Err(anyhow!(
                "Both an enum and an entity type definition '{}' exist in schema",
                name
            )),
            (Some(entity), None) => Ok(TypeDef::Entity(entity)),
            (None, Some(entity)) => Ok(TypeDef::Enum(entity)),
        }
    }

    fn check_related_type_defs_exist(self) -> anyhow::Result<Self> {
        for entity in self.entities.values() {
            for rel in entity.get_relationships() {
                match &rel {
                    Relationship::TypeDef { name } => {
                        let _ = self.try_get_type_def(name)?;
                    }
                    Relationship::DerivedFrom {
                        name,
                        derived_from_field,
                    } => {
                        let type_def = self.try_get_type_def(name)?;

                        match type_def {
                            TypeDef::Enum(_) => Err(anyhow!(
                                "Cannot derive field {derived_from_field} from enum {name}. \
                                 derivedFrom is intended to be used with Entity type definitions"
                            ))?,
                            TypeDef::Entity(derived_entity) => {
                                match derived_entity.fields.get(derived_from_field) {
                                    None => Err(anyhow!(
                                        "Derived field {derived_from_field} does not exist on \
                                         entity {name}."
                                    ))?,
                                    Some(field) => match field.field_type.get_underlying_scalar() {
                                        GqlScalar::Custom(name) if name == entity.name => (),
                                        GqlScalar::ID | GqlScalar::String => (),
                                        _ => Err(anyhow!(
                                            "Derived field '{derived_from_field}' on entity \
                                             '{name}' must either be an ID, String, or an Object \
                                             relationship with Entity '{}'",
                                            entity.name
                                        ))?,
                                    },
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(self)
    }

    /// For all entities validate the defined field types.
    ///
    /// This function will return an error if there is a defined related type where the type does
    /// not exist on the schema.
    fn validate_entity_field_types(self) -> anyhow::Result<Self> {
        for e in self.entities.values() {
            e.validate_field_types(&self)?;
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GraphQLEnum {
    pub name: String,
    pub values: Vec<String>,
}

impl GraphQLEnum {
    pub fn new(name: String, values: Vec<String>) -> anyhow::Result<Self> {
        Self { name, values }.valididate()
    }

    fn valididate(self) -> anyhow::Result<Self> {
        self.check_duplicate_values()?.check_valid_postgres_name()
    }

    fn check_duplicate_values(self) -> anyhow::Result<Self> {
        let mut value_set: HashSet<String> = self.values.clone().into_iter().collect();

        let duplicate_values = self
            .values
            .clone()
            .into_iter()
            .filter(|value| value_set.insert(value.clone()))
            .collect::<Vec<_>>();

        if !duplicate_values.is_empty() {
            Err(anyhow!(
                "EE212: Schema enum has duplicate values. Enum: {}, duplicate values: {}",
                self.name,
                duplicate_values.join(", ")
            ))
        } else {
            Ok(self)
        }
    }

    fn check_valid_postgres_name(self) -> anyhow::Result<Self> {
        let values_to_check = vec![vec![self.name.clone()], self.values.clone()].concat();
        let invalid_names = values_to_check
            .into_iter()
            .filter(|v| !is_valid_postgres_db_name(v))
            .collect::<Vec<_>>();

        if !invalid_names.is_empty() {
            Err(anyhow!(
                "EE214: Schema contains the enum names and/or values that does not match the \
                 following pattern: It must start with a letter. It can only contain letters, \
                 numbers, and underscores (no spaces). It must have a maximum length of 63 \
                 characters. Invalid names: '{}'",
                invalid_names.join(", ")
            ))
        } else {
            Ok(self)
        }
    }
    fn from_enum(enm: &EnumType<String>) -> anyhow::Result<Self> {
        let name = enm.name.clone();
        let values = enm
            .values
            .iter()
            .map(|value| value.name.clone())
            .collect::<Vec<String>>();
        Self::new(name, values)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    pub name: String,
    pub fields: HashMap<String, Field>,
}

impl Entity {
    fn new(name: String, fields: Vec<Field>) -> anyhow::Result<Self> {
        let fields = unique_hashmap::from_vec_no_duplicates(
            fields.into_iter().map(|f| (f.name.clone(), f)).collect(),
        )
        .context(format!(
            "Found fields with duplicate names on Entity {name}"
        ))?;
        Ok(Self { name, fields })
    }

    fn from_object(obj: &ObjectType<String>) -> anyhow::Result<Self> {
        let name = obj.name.clone();

        let fields = obj
            .fields
            .iter()
            .map(|f| Field::from_obj_field(f))
            .collect::<anyhow::Result<_>>()
            .context("Failed contsructing fields")?;

        Self::new(name, fields)
    }

    pub fn get_relationships(&self) -> Vec<Relationship> {
        let derived_from_fields: Vec<Relationship> = self
            .fields
            .values()
            .filter_map(|f| match &f.field_type {
                FieldType::DerivedFromField {
                    entity_name,
                    derived_from_field,
                } => Some(Relationship::DerivedFrom {
                    name: entity_name.clone(),
                    derived_from_field: derived_from_field.clone(),
                }),
                _ => None,
            })
            .collect();
        let type_defs: Vec<Relationship> = self
            .fields
            .values()
            .filter_map(|f| f.get_relationship())
            .collect();

        vec![derived_from_fields, type_defs].concat()
    }

    pub fn get_related_entities<'a>(
        &'a self,
        other_entities: &'a EntityMap,
        gql_enums: &GraphQlEnumMap,
    ) -> anyhow::Result<Vec<(&'a Field, &'a Self)>> {
        let required_entities_with_field = self
            .fields
            .values()
            .filter_map(|field| {
                let gql_scalar = field.field_type.get_underlying_scalar();
                if let GqlScalar::Custom(name) = gql_scalar {
                    if gql_enums.contains_key(&name) {
                        None
                    } else {
                        let field_and_entity = other_entities
                            .get(&name)
                            .map(|entity| (field, entity))
                            .ok_or_else(|| anyhow!("Entity {} does not exist", name));
                        Some(field_and_entity)
                    }
                } else {
                    None
                }
            })
            .collect::<anyhow::Result<_>>()?;

        Ok(required_entities_with_field)
    }

    /// Validate each field type in an the given entity
    ///
    /// This function will return an error if there is a defined related type where the type does
    /// not exist on the schema.
    fn validate_field_types(&self, schema: &Schema) -> anyhow::Result<()> {
        for field in self.fields.values() {
            field.validate_field_type(schema)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Field {
    pub name: String,
    pub field_type: FieldType,
}

impl Field {
    fn from_obj_field(field: &ObjField<String>) -> anyhow::Result<Self> {
        //Get all gql derictives labeled @derivedFrom
        let derived_from_directives = field
            .directives
            .iter()
            .filter(|directive| directive.name == "derivedFrom")
            .collect::<Vec<&Directive<'_, String>>>();

        //Do not allow for multiple @derivedFrom directives
        //If this step is not important and we are fine with just taking the first one
        //in the case of multiple we can just use a find rather than a filter method above
        if derived_from_directives.len() > 1 {
            let msg = anyhow!(
                "EE202: Cannot use more than one @derivedFrom directive at field {}",
                field.name
            );
            return Err(msg);
        }

        let maybe_derived_from_directive = derived_from_directives.get(0);
        let derived_from_field = match maybe_derived_from_directive {
            None => None,
            Some(d) => {
                let field_arg = d.arguments.iter().find(|a| a.0 == "field").ok_or_else(|| {
                    anyhow!(
                        "EE203: No 'field' argument supplied to @derivedFrom directive on field {}",
                        field.name
                    )
                })?;
                match &field_arg.1 {
                    Value::String(val) => Some(val.clone()),
                    _ => Err(anyhow!(
                        "EE204: 'field' argument in @derivedFrom directive on field {} needs to \
                         contain a string",
                        field.name
                    ))?,
                }
            }
        };

        let field_type = FieldType::from_obj_field_type(&field.field_type, derived_from_field)
            .context(format!("Failed parsing field {}", field.name))?;

        Ok(Field {
            name: field.name.clone(),
            field_type,
        })
    }

    fn get_relationship(&self) -> Option<Relationship> {
        match self.field_type.get_underlying_scalar() {
            GqlScalar::Custom(name) => Some(Relationship::TypeDef { name: name.clone() }),
            _ => None,
        }
    }

    fn validate_field_type(&self, schema: &Schema) -> anyhow::Result<()> {
        self.field_type.validate_type(schema)
    }

    pub fn get_relational_key(&self, schema: &Schema) -> anyhow::Result<String> {
        match &self.field_type {
            FieldType::DerivedFromField {
                derived_from_field,
                entity_name,
            } => {
                let entity_field = schema
                    .entities
                    .get(entity_name)
                    .ok_or_else(|| anyhow!("Unexpected, entity {entity_name} does not exist"))?
                    .fields
                    .get(derived_from_field)
                    .ok_or_else(|| {
                        anyhow!(
                            "Unexpected, field {derived_from_field} does not exist on entity \
                             {entity_name}"
                        )
                    })?;

                match entity_field.field_type.get_underlying_scalar() {
                    //In the case where there is a recipracol lookup, the actual
                    //underlying field contains _id at the end
                    GqlScalar::Custom(name)
                        if matches!(schema.try_get_type_def(&name)?, TypeDef::Entity(_)) =>
                    {
                        Ok(format!("{derived_from_field}_id"))
                    }
                    //In the case where its just an an ID or a string,
                    //just keep the the field as is from what was
                    //defined in @derivedFrom
                    GqlScalar::ID | GqlScalar::String => Ok(derived_from_field.clone()),
                    _ => Err(anyhow!(
                        "Unexpected, derived from field is neither an ID, String or bidirectional \
                         relationship"
                    ))?,
                }
            }

            FieldType::RegularField(_) => Ok(self.name.clone()),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum RescriptType {
    ID,
    Int,
    Float,
    BigInt,
    Address,
    String,
    Bool,
    EnumVariant(CapitalizedOptions),
    Array(Box<RescriptType>),
    Option(Box<RescriptType>),
    Tuple(Vec<RescriptType>),
}

impl RescriptType {
    pub fn to_string_decoded_skar(&self) -> String {
        match self {
            RescriptType::Array(inner_type) => format!(
                "array<HyperSyncClient.Decoder.decodedSolType<{}>>",
                inner_type.to_string_decoded_skar()
            ),
            RescriptType::Tuple(inner_types) => {
                let inner_types_str = inner_types
                    .iter()
                    .map(|inner_type| inner_type.to_string_decoded_skar())
                    .collect::<Vec<String>>()
                    .join(", ");
                format!(
                    "HyperSyncClient.Decoder.decodedSolType<({})>",
                    inner_types_str
                )
            }
            v => {
                format!("HyperSyncClient.Decoder.decodedSolType<{}>", v.to_string())
            }
        }
    }

    fn to_string(&self) -> String {
        match self {
            RescriptType::Int => "int".to_string(),
            RescriptType::Float => "float".to_string(),
            RescriptType::BigInt => "Ethers.BigInt.t".to_string(),
            RescriptType::Address => "Ethers.ethAddress".to_string(),
            RescriptType::String => "string".to_string(),
            RescriptType::ID => "id".to_string(),
            RescriptType::Bool => "bool".to_string(),
            RescriptType::Array(inner_type) => {
                format!("array<{}>", inner_type.to_string())
            }
            RescriptType::Option(inner_type) => {
                format!("option<{}>", inner_type.to_string())
            }
            RescriptType::Tuple(inner_types) => {
                let inner_types_str = inner_types
                    .iter()
                    .map(|inner_type| inner_type.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                format!("({})", inner_types_str)
            }
            RescriptType::EnumVariant(enum_name) => format!("Enums.{}", &enum_name.uncapitalized),
        }
    }

    pub fn get_default_value_rescript(&self) -> String {
        match self {
            RescriptType::Int => "0".to_string(),
            RescriptType::Float => "0.0".to_string(),
            RescriptType::BigInt => "Ethers.BigInt.zero".to_string(),
            RescriptType::Address => "Ethers.Addresses.defaultAddress".to_string(),
            RescriptType::String => "\"foo\"".to_string(),
            RescriptType::ID => "\"my_id\"".to_string(),
            RescriptType::Bool => "false".to_string(),
            RescriptType::Array(_) => "[]".to_string(),
            RescriptType::Option(_) => "None".to_string(),
            RescriptType::EnumVariant(enum_name) => {
                format!("Enums.{}Default", &enum_name.uncapitalized)
            }
            RescriptType::Tuple(inner_types) => {
                let inner_types_str = inner_types
                    .iter()
                    .map(|inner_type| inner_type.get_default_value_rescript())
                    .collect::<Vec<String>>()
                    .join(", ");

                format!("({})", inner_types_str)
            }
        }
    }

    pub fn get_default_value_non_rescript(&self) -> String {
        match self {
            RescriptType::Int | RescriptType::Float => "0".to_string(),
            RescriptType::BigInt => "0n".to_string(),
            RescriptType::Address => "Addresses.defaultAddress".to_string(),
            RescriptType::String => "\"foo\"".to_string(),
            RescriptType::ID => "\"my_id\"".to_string(),
            RescriptType::Bool => "false".to_string(),
            RescriptType::Array(_) => "[]".to_string(),
            RescriptType::Option(_) => "null".to_string(),
            RescriptType::EnumVariant(enum_name) => format!("{}Default", &enum_name.uncapitalized),
            RescriptType::Tuple(inner_types) => {
                let inner_types_str = inner_types
                    .iter()
                    .map(|inner_type| inner_type.get_default_value_non_rescript())
                    .collect::<Vec<String>>()
                    .join(", ");

                format!("[{}]", inner_types_str)
            }
        }
    }
}

impl Display for RescriptType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

///Implementation of Serialize allows handlebars get a stringified
///version of the string representation of the rescript type
impl Serialize for RescriptType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as display value
        self.to_string().serialize(serializer)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UserDefinedFieldType {
    Single(GqlScalar),
    ListType(Box<UserDefinedFieldType>),
    NonNullType(Box<UserDefinedFieldType>),
}

impl UserDefinedFieldType {
    fn from_obj_field_type(obj_field_type: &ObjType<'_, String>) -> Self {
        match obj_field_type {
            ObjType::NamedType(name) => UserDefinedFieldType::Single(GqlScalar::from_str(name)),
            ObjType::NonNullType(obj_field_type) => UserDefinedFieldType::NonNullType(Box::new(
                Self::from_obj_field_type(obj_field_type),
            )),
            ObjType::ListType(obj_field_type) => {
                UserDefinedFieldType::ListType(Box::new(Self::from_obj_field_type(obj_field_type)))
            }
        }
    }

    pub fn validate_type(&self, schema: &Schema) -> anyhow::Result<()> {
        match self {
            Self::Single(_) => Ok(()),
            Self::ListType(field_type) => match field_type.as_ref() {
                //Postgres doesn't support nullable types inside of arrays
                Self::NonNullType(inner_field_type) => match inner_field_type.as_ref() {
                    //Don't allow non derived from enity relationships inside arrays
                    Self::Single(GqlScalar::Custom(name))
                        if matches!(schema.try_get_type_def(name)?, TypeDef::Entity(_)) =>
                    {
                        Err(anyhow!(
                            "EE211: Arrays of entities is unsupported. Please use one of the \
                             methods for referencing entities outlined in the docs. The entity \
                             being referenced in the array is '{}'.",
                            name
                        ))?
                    }
                    _ => field_type.validate_type(schema),
                },
                Self::Single(gql_scalar) => Err(anyhow!(
                    "EE208: Nullable scalars inside lists are unsupported. Please include a '!' \
                     after your '{}' scalar",
                    gql_scalar
                ))?,
                Self::ListType(_) => Err(anyhow!(
                    "EE209: Nullable multidimensional lists types are unsupported,please include \
                     a '!' for your inner list type eg. [[Int!]!]"
                ))?,
            },
            Self::NonNullType(field_type) => match field_type.as_ref() {
                Self::NonNullType(_) => Err(anyhow!(
                    "Nested Not Null types are unsupported. Please remove any sequential '!' \
                     symbols after types in schema"
                )),
                _ => field_type.validate_type(schema),
            },
        }
    }

    pub fn to_postgres_type<'a>(&'a self, schema: &'a Schema) -> anyhow::Result<String> {
        match self {
            Self::Single(gql_scalar) => gql_scalar.to_postgres_type(schema),
            Self::ListType(field_type) => match field_type.as_ref() {
                Self::NonNullType(non_null) => {
                    Ok(format!("{}[]", non_null.to_postgres_type(schema)?))
                }

                _ => Err(anyhow!(
                    "Unexpected invalid case. Only Not Null List values can be valid valid."
                )), //This case should be caught during validation. It is unexpected that we would
                    //it it here
            },
            Self::NonNullType(field_type) => {
                Ok(format!("{} NOT NULL", field_type.to_postgres_type(schema)?))
            }
        }
    }

    pub fn is_optional(&self) -> bool {
        !matches!(self, Self::NonNullType(_))
    }

    pub fn is_array(&self) -> bool {
        match self {
            Self::ListType(_) => true,
            Self::NonNullType(field_type) => field_type.is_array(),
            Self::Single(_) => false,
        }
    }

    pub fn to_rescript_type(&self, schema: &Schema) -> anyhow::Result<RescriptType> {
        let composed_type_name = match self {
            //Only types in here should be non optional
            Self::NonNullType(field_type) => match field_type.as_ref() {
                Self::Single(gql_scalar) => gql_scalar.to_rescript_type(schema)?,
                Self::ListType(field_type) => {
                    RescriptType::Array(Box::new(field_type.to_rescript_type(schema)?))
                }
                //This case shouldn't happen, and should recurse without adding any types if so
                //A double non null would be !! in gql
                Self::NonNullType(field_type) => field_type.to_rescript_type(schema)?,
            },
            //If we match this case it missed the non null path entirely and should be optional
            Self::Single(gql_scalar) => {
                RescriptType::Option(Box::new(gql_scalar.to_rescript_type(schema)?))
            }
            //If we match this case it missed the non null path entirely and should be optional
            Self::ListType(field_type) => RescriptType::Option(Box::new(RescriptType::Array(
                Box::new(field_type.to_rescript_type(schema)?),
            ))),
        };
        Ok(composed_type_name)
    }

    fn get_underlying_scalar(&self) -> GqlScalar {
        match self {
            Self::Single(gql_scalar) => gql_scalar.clone(),
            Self::ListType(field_type) | Self::NonNullType(field_type) => {
                field_type.get_underlying_scalar()
            }
        }
    }

    pub fn is_entity_field(&self, schema: &Schema) -> anyhow::Result<bool> {
        self.get_underlying_scalar().is_entity(schema)
    }

    fn to_string(&self) -> String {
        match &self {
            Self::Single(gql_scalar) => gql_scalar.to_string(),
            Self::ListType(field_type) => format!("[{}]", field_type.to_string()),
            Self::NonNullType(field_type) => format!("{}!", field_type.to_string()),
        }
    }

    /// Returns the name of the entity when @derivedFrom derivtive is used
    /// Returns None in the case that it does not conform to the correct
    /// structure of a derived entity
    fn get_name_of_derived_from_entity(&self) -> Option<String> {
        match self {
            Self::NonNullType(f) => match f.as_ref() {
                Self::ListType(f) => match f.as_ref() {
                    Self::NonNullType(f) => match f.as_ref() {
                        Self::Single(GqlScalar::Custom(name)) => Some(name.clone()),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }
    }

    pub fn from_ethabi_type(abi_type: &EthAbiParamType) -> anyhow::Result<Self> {
        match abi_type {
            EthAbiParamType::Uint(_size) | EthAbiParamType::Int(_size) => {
                Ok(Self::NonNullType(Box::new(Self::Single(GqlScalar::BigInt))))
            }
            EthAbiParamType::Bool => Ok(Self::NonNullType(Box::new(Self::Single(
                GqlScalar::Boolean,
            )))),
            EthAbiParamType::Address
            | EthAbiParamType::Bytes
            | EthAbiParamType::String
            | EthAbiParamType::FixedBytes(_) => {
                Ok(Self::NonNullType(Box::new(Self::Single(GqlScalar::String))))
            }
            EthAbiParamType::Array(abi_type) | EthAbiParamType::FixedArray(abi_type, _) => {
                let inner_type = Self::from_ethabi_type(abi_type)?;
                Ok(Self::NonNullType(Box::new(Self::ListType(Box::new(
                    inner_type,
                )))))
            }
            EthAbiParamType::Tuple(_abi_types) => Err(anyhow!(
                "Tuples are not handled currently using contract import."
            )),
        }
    }
}

// Implement the Display trait for the custom struct
impl fmt::Display for UserDefinedFieldType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl Serialize for UserDefinedFieldType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FieldType {
    DerivedFromField {
        entity_name: String,
        derived_from_field: String,
    },
    RegularField(UserDefinedFieldType),
}

impl FieldType {
    fn to_user_defined_field_type(&self) -> UserDefinedFieldType {
        match self {
            Self::RegularField(t) => t.clone(),
            Self::DerivedFromField { entity_name, .. } => {
                use UserDefinedFieldType::*;
                NonNullType(Box::new(ListType(Box::new(NonNullType(Box::new(Single(
                    GqlScalar::Custom(entity_name.clone()),
                )))))))
            }
        }
    }

    fn from_obj_field_type(
        obj_field_type: &ObjType<'_, String>,
        derived_from_field: Option<String>,
    ) -> anyhow::Result<Self> {
        let field_type = UserDefinedFieldType::from_obj_field_type(obj_field_type);

        match derived_from_field {
            None => Ok(Self::RegularField(field_type)),
            Some(derived_from_field) => match field_type.get_name_of_derived_from_entity() {
                None => {
                    let example_str = Self::DerivedFromField {
                        entity_name: "<MY_ENTITY>".to_string(),
                        derived_from_field,
                    }
                    .to_string();

                    Err(anyhow!(
                        "Field marked with @derivedFrom directive does not meet the required \
                         structure. Field should contain a non nullable list of non nullable \
                         entities for example: {example_str}"
                    ))
                }
                Some(entity_name) => Ok(Self::DerivedFromField {
                    entity_name,
                    derived_from_field,
                }),
            },
        }
    }

    pub fn validate_type(&self, schema: &Schema) -> anyhow::Result<()> {
        match self {
            Self::DerivedFromField { .. } => Ok(()), //Already validated
            Self::RegularField(t) => t.validate_type(schema),
        }
    }

    pub fn to_postgres_type<'a>(&'a self, schema: &'a Schema) -> anyhow::Result<String> {
        self.to_user_defined_field_type().to_postgres_type(schema)
    }

    pub fn is_optional(&self) -> bool {
        self.to_user_defined_field_type().is_optional()
    }

    pub fn is_derived_from(&self) -> bool {
        matches!(self, Self::DerivedFromField { .. })
    }

    pub fn is_array(&self) -> bool {
        match self {
            Self::DerivedFromField { .. } => true,
            Self::RegularField(t) => t.is_array(),
        }
    }

    pub fn to_rescript_type(&self, schema: &Schema) -> anyhow::Result<RescriptType> {
        self.to_user_defined_field_type().to_rescript_type(schema)
    }

    fn get_underlying_scalar(&self) -> GqlScalar {
        self.to_user_defined_field_type().get_underlying_scalar()
    }

    pub fn is_entity_field(&self, schema: &Schema) -> anyhow::Result<bool> {
        self.to_user_defined_field_type().is_entity_field(schema)
    }

    fn to_string(&self) -> String {
        match self {
            Self::DerivedFromField { entity_name, .. } => {
                let field_str = self.to_user_defined_field_type().to_string();
                format!("{field_str} @derivedFrom(field: \"{entity_name}\")")
            }
            Self::RegularField(t) => t.to_string(),
        }
    }

    pub fn from_ethabi_type(abi_type: &EthAbiParamType) -> anyhow::Result<Self> {
        Ok(Self::RegularField(UserDefinedFieldType::from_ethabi_type(
            abi_type,
        )?))
    }
}

// Implement the Display trait for the custom struct
impl fmt::Display for FieldType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl Serialize for FieldType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

#[subenum(BuiltInGqlScalar, AdditionalGqlScalar)]
#[derive(Debug, Clone, PartialEq, strum_macros::Display, Eq, Hash)]
pub enum GqlScalar {
    #[subenum(BuiltInGqlScalar)]
    ID,
    #[subenum(BuiltInGqlScalar)]
    String,
    #[subenum(BuiltInGqlScalar)]
    Int,
    #[subenum(BuiltInGqlScalar)]
    Float,
    #[subenum(BuiltInGqlScalar)]
    Boolean,
    #[subenum(AdditionalGqlScalar)]
    BigInt,
    #[subenum(AdditionalGqlScalar)]
    Bytes,
    Custom(String),
}

#[derive(Debug, Clone)]
pub enum Relationship {
    TypeDef {
        name: String,
    },
    DerivedFrom {
        name: String,
        derived_from_field: String,
    },
}

impl GqlScalar {
    fn is_entity(&self, schema: &Schema) -> anyhow::Result<bool> {
        match self {
            GqlScalar::Custom(name) => {
                Ok(matches!(schema.try_get_type_def(name)?, TypeDef::Entity(_)))
            }
            _ => Ok(false),
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "ID" => GqlScalar::ID,
            "String" => GqlScalar::String,
            "Int" => GqlScalar::Int,
            "Float" => GqlScalar::Float, // Should we allow this type? Rounding issues will abound.
            "Boolean" => GqlScalar::Boolean,
            "BigInt" => GqlScalar::BigInt, // NOTE: we aren't setting precision and scale - see (8.1.2) https://www.postgresql.org/docs/current/datatype-numeric.html
            "Bytes" => GqlScalar::Bytes,
            name => GqlScalar::Custom(name.to_string()),
        }
    }
    fn to_postgres_type(&self, schema: &Schema) -> anyhow::Result<String> {
        let converted = match self {
            GqlScalar::ID => "text",
            GqlScalar::String => "text",
            GqlScalar::Int => "integer",
            GqlScalar::Float => "numeric", // Should we allow this type? Rounding issues will abound.
            GqlScalar::Boolean => "boolean",
            GqlScalar::Bytes => "text",
            GqlScalar::BigInt => "numeric", // NOTE: we aren't setting precision and scale - see (8.1.2) https://www.postgresql.org/docs/current/datatype-numeric.html
            GqlScalar::Custom(name) => match schema.try_get_type_def(name)? {
                TypeDef::Entity(_) => "text",
                TypeDef::Enum(_) => name.as_str(),
            },
        };
        Ok(converted.to_string())
    }

    fn to_rescript_type(&self, schema: &Schema) -> anyhow::Result<RescriptType> {
        let res_type = match self {
            GqlScalar::ID => RescriptType::ID,
            GqlScalar::String => RescriptType::String,
            GqlScalar::Int => RescriptType::Int,
            GqlScalar::BigInt => RescriptType::BigInt,
            GqlScalar::Float => RescriptType::Float,
            GqlScalar::Bytes => RescriptType::String,
            GqlScalar::Boolean => RescriptType::Bool,
            GqlScalar::Custom(name) => match schema.try_get_type_def(name)? {
                TypeDef::Entity(_) => RescriptType::ID,
                TypeDef::Enum(_) => RescriptType::EnumVariant(name.to_capitalized_options()),
            },
        };
        Ok(res_type)
    }
}

#[cfg(test)]
mod tests {
    use super::{Entity, FieldType, GqlScalar, GraphQLEnum, Schema, UserDefinedFieldType};

    #[test]
    fn gql_type_to_rescript_type_string() {
        let empty_schema = Schema::empty();
        let rescript_type = UserDefinedFieldType::Single(GqlScalar::String)
            .to_rescript_type(&empty_schema)
            .expect("expected rescript option string");

        assert_eq!(rescript_type.to_string(), "option<string>".to_owned());
    }

    #[test]
    fn gql_type_to_rescript_type_int() {
        let empty_schema = Schema::empty();
        let rescript_type = UserDefinedFieldType::Single(GqlScalar::Int)
            .to_rescript_type(&empty_schema)
            .expect("expected rescript option string");

        assert_eq!(rescript_type.to_string(), "option<int>".to_owned());
    }

    #[test]
    fn gql_type_to_rescript_type_non_null_int() {
        let empty_schema = Schema::empty();
        let rescript_type = UserDefinedFieldType::NonNullType(Box::new(
            UserDefinedFieldType::Single(GqlScalar::Int),
        ))
        .to_rescript_type(&empty_schema)
        .expect("expected rescript type string");

        assert_eq!(rescript_type.to_string(), "int".to_owned());
    }

    #[test]
    fn gql_type_to_rescript_type_non_null_array() {
        let empty_schema = Schema::empty();
        let rescript_type = UserDefinedFieldType::NonNullType(Box::new(
            UserDefinedFieldType::ListType(Box::new(UserDefinedFieldType::NonNullType(Box::new(
                UserDefinedFieldType::Single(GqlScalar::Int),
            )))),
        ))
        .to_rescript_type(&empty_schema)
        .expect("expected rescript type string");

        assert_eq!(rescript_type.to_string(), "array<int>".to_owned());
    }

    #[test]
    fn gql_type_to_rescript_type_null_array_int() {
        let empty_schema = Schema::empty();

        let rescript_type =
            UserDefinedFieldType::ListType(Box::new(UserDefinedFieldType::Single(GqlScalar::Int)))
                .to_rescript_type(&empty_schema)
                .expect("expected rescript type string");

        assert_eq!(
            rescript_type.to_string(),
            "option<array<option<int>>>".to_owned()
        );
    }

    #[test]
    fn gql_type_to_rescript_type_entity() {
        let test_entity_string = String::from("TestEntity");
        let test_entity = Entity::new(test_entity_string.clone(), vec![]).unwrap();
        let schema = Schema::new(vec![test_entity], vec![]).unwrap();
        let rescript_type = UserDefinedFieldType::Single(GqlScalar::Custom(test_entity_string))
            .to_rescript_type(&schema)
            .expect("expected rescript type string");

        assert_eq!(rescript_type.to_string(), "option<id>".to_owned());
    }

    #[test]
    fn gql_type_to_rescript_type_enum() {
        let name = String::from("TestEnum");
        let test_enum = GraphQLEnum::new(name.clone(), vec![]).unwrap();
        let schema = Schema::new(vec![], vec![test_enum]).unwrap();
        let rescript_type = UserDefinedFieldType::Single(GqlScalar::Custom(name))
            .to_rescript_type(&schema)
            .expect("expected rescript type string");

        assert_eq!(
            rescript_type.to_string(),
            "option<Enums.testEnum>".to_owned()
        );
    }

    #[test]
    fn field_type_is_optional_test() {
        let test_scalar = GqlScalar::Custom(String::from("TestEntity"));
        let test_field_type = UserDefinedFieldType::Single(test_scalar);
        assert!(
            test_field_type.is_optional(),
            "single field should have been optional"
        );

        // ListType:
        let test_list_type = UserDefinedFieldType::ListType(Box::new(test_field_type));
        assert!(
            test_list_type.is_optional(),
            "list field should have been optional"
        );

        // NonNullType
        let gql_array_non_null_type = UserDefinedFieldType::NonNullType(Box::new(test_list_type));
        assert!(
            !gql_array_non_null_type.is_optional(),
            "non-null field should not be optioonal"
        );
    }

    fn get_field_type_helper_with_additional(
        gql_field_str: &str,
        enum_types: Vec<GraphQLEnum>,
    ) -> FieldType {
        let enum_type_defs: String = enum_types
            .iter()
            .map(|e| format!("enum {} {{\n{}\n}}", e.name, e.values.join("\n")))
            .collect::<Vec<_>>()
            .join("\n");

        println!("{enum_type_defs}");
        let schema_string = format!(
            r#"
        type TestEntity {{
          test_field: {gql_field_str}
        }}
        {enum_type_defs}
        "#,
        );
        let schema_doc = graphql_parser::schema::parse_schema::<String>(&schema_string).unwrap();

        let schema = Schema::from_document(schema_doc).expect("bad schema");

        let test_field = schema
            .entities
            .get("TestEntity")
            .expect("No test entity in schema")
            .fields
            .get("test_field")
            .expect("No field test_field on entity")
            .clone();

        test_field.field_type
    }
    fn get_field_type_helper(gql_field_str: &str) -> FieldType {
        get_field_type_helper_with_additional(gql_field_str, vec![])
    }

    fn gql_type_to_postgres_type_test_helper(gql_field_str: &str) -> String {
        let field_type = get_field_type_helper(gql_field_str);
        let empty_schema = Schema::empty();
        field_type
            .to_postgres_type(&empty_schema)
            .expect("unable to get postgres type")
    }

    #[test]
    fn gql_enum_type_to_postgres_type() {
        let name = String::from("TestEnum");
        let test_enum = GraphQLEnum::new(name.clone(), vec!["TEST_VALUE".to_string()]).unwrap();
        let field_type =
            get_field_type_helper_with_additional("TestEnum!", vec![test_enum.clone()]);
        let schema = Schema::new(vec![], vec![test_enum]).unwrap();
        let pg_type = field_type
            .to_postgres_type(&schema)
            .expect("unable to get postgres type");
        assert_eq!(pg_type, "TestEnum NOT NULL");
    }

    #[test]
    fn gql_single_not_null_array_to_pg_type() {
        let gql_type = "[String!]!";
        let pg_type = gql_type_to_postgres_type_test_helper(gql_type);
        assert_eq!(pg_type, "text[] NOT NULL");
    }

    #[test]
    fn gql_multi_not_null_array_to_pg_type() {
        let gql_type = "[[Int!]!]!";
        let pg_type = gql_type_to_postgres_type_test_helper(gql_type);
        assert_eq!(pg_type, "integer[][] NOT NULL");
    }

    #[test]
    #[should_panic]
    fn gql_single_nullable_array_to_pg_type_should_panic() {
        let gql_type = "[Int]!"; //Nested lists need to be not nullable
        gql_type_to_postgres_type_test_helper(gql_type);
    }

    #[test]
    #[should_panic]
    fn gql_multi_nullable_array_to_pg_type_should_panic() {
        let gql_type = "[[Int!]]!"; //Nested lists need to be not nullable
        gql_type_to_postgres_type_test_helper(gql_type);
    }

    #[test]
    fn test_nullability_to_string() {
        use UserDefinedFieldType::{ListType, NonNullType, Single};
        let scalar = NonNullType(Box::new(ListType(Box::new(Single(GqlScalar::Int)))));

        let expected_output = "[Int]!".to_string();

        assert_eq!(scalar.to_string(), expected_output);
    }

    #[test]
    fn gql_type_to_rescript_nullable() {
        let field_type = get_field_type_helper("Int");

        let empty_schema = Schema::empty();
        let rescript_type = field_type.to_rescript_type(&empty_schema).unwrap();
        assert_eq!("option<int>".to_string(), rescript_type.to_string());
    }

    #[test]
    #[ignore = "We don't support list types with nullable scalars due to postgres so skipping this"]
    fn gql_type_to_rescript_array_nullable_string() {
        let field_type = get_field_type_helper("[String]!");

        let empty_schema = Schema::empty();
        let rescript_type = field_type.to_rescript_type(&empty_schema).unwrap();
        assert_eq!(
            "array<option<string>>".to_string(),
            rescript_type.to_string()
        );
    }
}
