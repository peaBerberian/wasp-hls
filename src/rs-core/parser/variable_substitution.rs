use super::attribute_list::{parse_comma_separated_list, parse_quoted_string, AttributeListIter};
use crate::utils::url::Url;
use std::{borrow::Cow, collections::HashMap};

/// Errors linked to HLS's EXT-X-DEFINE - based variable substitution
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum VariableDefinitionError {
    /// The given variable key was defined multiple times
    DuplicateName,
    /// The imported variable was not found in the banks of declared variable names
    MissingImportedVariable,
    /// The imported variable relied on a Query string params that did not exist
    MissingQueryParam,
    /// The variable name contained an un-authorized character
    InvalidName,
    /// Query params that may be used by variable definitions need to be percent decoded.
    /// This error arises if the percent encoding was invalid.
    InvalidPercentEncoding,
    /// The variable value contained an un-authorized character
    InvalidValue,
    /// The definition or usage of a variable was invalid.
    /// TODO: different errors?
    InvalidDefinition,
}

/// Store all variable definitions parsed until now and allow variable substitution
/// of parsed attributes.
#[derive(Clone, Debug, Default)]
pub(crate) struct VariableStore {
    variables: HashMap<String, String>,
    query_params: HashMap<String, String>,
}

impl VariableStore {
    /// Create a new `VariableStore`. Rely on the corresponding playlist's URL
    /// because variable definitions might rely on the query string.
    pub(crate) fn from_url(url: &Url) -> Self {
        Self {
            variables: HashMap::new(),
            query_params: parse_query_params(url),
        }
    }

    /// Define a new variable with the given name and value.
    pub(crate) fn define(
        &mut self,
        name: String,
        value: String,
    ) -> Result<(), VariableDefinitionError> {
        validate_variable_name(&name)?;
        validate_variable_value(&value)?;
        if self.variables.contains_key(&name) {
            return Err(VariableDefinitionError::DuplicateName);
        }
        self.variables.insert(name, value);
        Ok(())
    }

    pub(crate) fn import(
        &mut self,
        name: &str,
        imported_variables: &HashMap<String, String>,
    ) -> Result<(), VariableDefinitionError> {
        validate_variable_name(name)?;
        let value = imported_variables
            .get(name)
            .ok_or(VariableDefinitionError::MissingImportedVariable)?;
        self.define(name.to_owned(), value.clone())
    }

    pub(crate) fn define_query_param(&mut self, name: &str) -> Result<(), VariableDefinitionError> {
        validate_variable_name(name)?;
        let value = self
            .query_params
            .get(name)
            .ok_or(VariableDefinitionError::MissingQueryParam)?
            .clone();
        self.define(name.to_owned(), value)
    }

    pub(crate) fn substitute<'a>(
        &self,
        value: &'a str,
    ) -> Result<Cow<'a, str>, VariableDefinitionError> {
        let Some(first_idx) = value.find("{$") else {
            return Ok(Cow::Borrowed(value));
        };

        let mut substituted = String::with_capacity(value.len());
        substituted.push_str(&value[..first_idx]);
        let mut offset = first_idx;
        while offset < value.len() {
            match value[offset..].find("{$") {
                None => {
                    substituted.push_str(&value[offset..]);
                    break;
                }
                Some(relative_start) => {
                    let start = offset + relative_start;
                    substituted.push_str(&value[offset..start]);
                    let after_start = start + 2;
                    let Some(relative_end) = value[after_start..].find('}') else {
                        return Err(VariableDefinitionError::InvalidDefinition);
                    };
                    let end = after_start + relative_end;
                    let variable_name = &value[after_start..end];
                    validate_variable_name(variable_name)?;
                    let variable_value = self
                        .variables
                        .get(variable_name)
                        .ok_or(VariableDefinitionError::InvalidDefinition)?;
                    substituted.push_str(variable_value);
                    offset = end + 1;
                }
            }
        }
        Ok(Cow::Owned(substituted))
    }

    pub(crate) fn definitions(&self) -> &HashMap<String, String> {
        &self.variables
    }
}

pub(crate) enum VariableDefinition {
    Name { name: String, value: String },
    Import { name: String },
    QueryParam { name: String },
}

pub(crate) fn parse_substituted_quoted_string<'a>(
    line: &'a str,
    value_start_offset: usize,
    variable_store: &VariableStore,
) -> Result<Cow<'a, str>, VariableDefinitionError> {
    let (parsed, _) = parse_quoted_string(line, value_start_offset);
    let parsed = parsed.map_err(|_| VariableDefinitionError::InvalidDefinition)?;
    variable_store.substitute(parsed)
}

pub(crate) fn parse_substituted_comma_separated_list<'a>(
    line: &'a str,
    value_start_offset: usize,
    variable_store: &VariableStore,
) -> Result<Vec<Cow<'a, str>>, VariableDefinitionError> {
    let (parsed, _) = parse_comma_separated_list(line, value_start_offset);
    let parsed = parsed.map_err(|_| VariableDefinitionError::InvalidDefinition)?;
    parsed
        .into_iter()
        .map(|value| variable_store.substitute(value))
        .collect()
}

pub(crate) fn parse_define_tag(line: &str) -> Result<VariableDefinition, VariableDefinitionError> {
    let mut name: Option<String> = None;
    let mut value: Option<String> = None;
    let mut import: Option<String> = None;
    let mut query_param: Option<String> = None;
    for item in AttributeListIter::new(line, "#EXT-X-DEFINE:".len()) {
        if !matches!(item.name, "NAME" | "VALUE" | "IMPORT" | "QUERYPARAM") {
            continue;
        }
        let (parsed_value, _) = parse_quoted_string(line, item.value_start_offset);
        let parsed_value = parsed_value.map_err(|_| VariableDefinitionError::InvalidDefinition)?;
        match item.name {
            "NAME" => name = Some(parsed_value.to_owned()),
            "VALUE" => value = Some(parsed_value.to_owned()),
            "IMPORT" => import = Some(parsed_value.to_owned()),
            "QUERYPARAM" => query_param = Some(parsed_value.to_owned()),
            _ => {}
        }
    }

    let alternatives_count = usize::from(name.is_some())
        + usize::from(import.is_some())
        + usize::from(query_param.is_some());
    if alternatives_count != 1 {
        return Err(VariableDefinitionError::InvalidDefinition);
    }

    if let Some(name) = name {
        if let Some(value) = value {
            validate_variable_name(&name)?;
            validate_variable_value(&value)?;
            Ok(VariableDefinition::Name { name, value })
        } else {
            Err(VariableDefinitionError::InvalidDefinition)
        }
    } else if let Some(name) = import {
        validate_variable_name(&name)?;
        Ok(VariableDefinition::Import { name })
    } else if let Some(name) = query_param {
        validate_variable_name(&name)?;
        Ok(VariableDefinition::QueryParam { name })
    } else {
        Err(VariableDefinitionError::InvalidDefinition)
    }
}

fn validate_variable_name(name: &str) -> Result<(), VariableDefinitionError> {
    if name.is_empty() {
        return Err(VariableDefinitionError::InvalidName);
    }
    if name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        Ok(())
    } else {
        Err(VariableDefinitionError::InvalidName)
    }
}

fn validate_variable_value(value: &str) -> Result<(), VariableDefinitionError> {
    if value.contains('"') || value.contains('\r') || value.contains('\n') {
        Err(VariableDefinitionError::InvalidValue)
    } else {
        Ok(())
    }
}

fn parse_query_params(url: &Url) -> HashMap<String, String> {
    let input = url.get_ref();
    let Some(query_start) = input.find('?') else {
        return HashMap::new();
    };
    let query = match input[query_start + 1..].find('#') {
        Some(hash_offset) => &input[query_start + 1..query_start + 1 + hash_offset],
        None => &input[query_start + 1..],
    };

    let mut result = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let Some(eq_idx) = pair.find('=') else {
            continue;
        };
        let key = &pair[..eq_idx];
        let val = &pair[eq_idx + 1..];
        if result.contains_key(key) {
            continue;
        }
        match percent_decode(val) {
            Ok(decoded) => {
                if validate_variable_value(&decoded).is_ok() {
                    result.insert(key.to_owned(), decoded);
                }
            }
            Err(_) => continue,
        }
    }
    result
}

fn percent_decode(input: &str) -> Result<String, VariableDefinitionError> {
    let mut result = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(VariableDefinitionError::InvalidPercentEncoding);
            }
            let high = from_hex(bytes[i + 1])?;
            let low = from_hex(bytes[i + 2])?;
            result.push(high * 16 + low);
            i += 3;
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(result).map_err(|_| VariableDefinitionError::InvalidPercentEncoding)
}

fn from_hex(byte: u8) -> Result<u8, VariableDefinitionError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(VariableDefinitionError::InvalidPercentEncoding),
    }
}
