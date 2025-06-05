// Modern Rust 2024 PDF extraction library
use adobe_cmap_parser::{ByteMapping, CodeRange, CIDRange};
use encoding_rs::UTF_16BE;
use euclid::{vec2, Transform2D};
use log::{debug, warn, error};
use lopdf::{
    content::Content,
    encryption::DecryptionError,
};
use std::{
    collections::HashMap,
    fmt::{self, Debug},
    marker::PhantomData,
    sync::Arc,
    slice::Iter,
    str,
};
use thiserror::Error;
use cff_parser::Table;

// Re-export lopdf for backward compatibility
pub use lopdf::*;

// Specific modules
mod core_fonts;
mod encodings;
mod glyphnames;
mod zapfglyphnames;

// Type definitions with proper naming
pub struct PdfSpace;
pub type PdfTransform = Transform2D<f64, PdfSpace, PdfSpace>;

/// Comprehensive error type for PDF operations
#[derive(Error, Debug)]
pub enum PdfError {
    #[error("Formatting error: {0}")]
    Format(#[from] fmt::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("PDF parsing error: {0}")]
    Parse(#[from] lopdf::Error),
    
    #[error("Invalid PDF structure: {0}")]
    InvalidStructure(String),
    
    #[error("Font error: {0}")]
    FontError(String),
    
    #[error("Encoding error: {0}")]
    EncodingError(String),
    
    #[error("Missing required field: {0}")]
    MissingField(String),
}

pub type PdfResult<T> = std::result::Result<T, PdfError>;

// Constants with proper naming convention
const PDF_DOC_ENCODING: &[u16] = &[
    0x0000, 0x0001, 0x0002, 0x0003, 0x0004, 0x0005, 0x0006, 0x0007, 0x0008,
    0x0009, 0x000a, 0x000b, 0x000c, 0x000d, 0x000e, 0x000f, 0x0010, 0x0011,
    0x0012, 0x0013, 0x0014, 0x0015, 0x0016, 0x0017, 0x02d8, 0x02c7, 0x02c6,
    0x02d9, 0x02dd, 0x02db, 0x02da, 0x02dc, 0x0020, 0x0021, 0x0022, 0x0023,
    0x0024, 0x0025, 0x0026, 0x0027, 0x0028, 0x0029, 0x002a, 0x002b, 0x002c,
    0x002d, 0x002e, 0x002f, 0x0030, 0x0031, 0x0032, 0x0033, 0x0034, 0x0035,
    0x0036, 0x0037, 0x0038, 0x0039, 0x003a, 0x003b, 0x003c, 0x003d, 0x003e,
    0x003f, 0x0040, 0x0041, 0x0042, 0x0043, 0x0044, 0x0045, 0x0046, 0x0047,
    0x0048, 0x0049, 0x004a, 0x004b, 0x004c, 0x004d, 0x004e, 0x004f, 0x0050,
    0x0051, 0x0052, 0x0053, 0x0054, 0x0055, 0x0056, 0x0057, 0x0058, 0x0059,
    0x005a, 0x005b, 0x005c, 0x005d, 0x005e, 0x005f, 0x0060, 0x0061, 0x0062,
    0x0063, 0x0064, 0x0065, 0x0066, 0x0067, 0x0068, 0x0069, 0x006a, 0x006b,
    0x006c, 0x006d, 0x006e, 0x006f, 0x0070, 0x0071, 0x0072, 0x0073, 0x0074,
    0x0075, 0x0076, 0x0077, 0x0078, 0x0079, 0x007a, 0x007b, 0x007c, 0x007d,
    0x007e, 0x0000, 0x2022, 0x2020, 0x2021, 0x2026, 0x2014, 0x2013, 0x0192,
    0x2044, 0x2039, 0x203a, 0x2212, 0x2030, 0x201e, 0x201c, 0x201d, 0x2018,
    0x2019, 0x201a, 0x2122, 0xfb01, 0xfb02, 0x0141, 0x0152, 0x0160, 0x0178,
    0x017d, 0x0131, 0x0142, 0x0153, 0x0161, 0x017e, 0x0000, 0x20ac, 0x00a1,
    0x00a2, 0x00a3, 0x00a4, 0x00a5, 0x00a6, 0x00a7, 0x00a8, 0x00a9, 0x00aa,
    0x00ab, 0x00ac, 0x0000, 0x00ae, 0x00af, 0x00b0, 0x00b1, 0x00b2, 0x00b3,
    0x00b4, 0x00b5, 0x00b6, 0x00b7, 0x00b8, 0x00b9, 0x00ba, 0x00bb, 0x00bc,
    0x00bd, 0x00be, 0x00bf, 0x00c0, 0x00c1, 0x00c2, 0x00c3, 0x00c4, 0x00c5,
    0x00c6, 0x00c7, 0x00c8, 0x00c9, 0x00ca, 0x00cb, 0x00cc, 0x00cd, 0x00ce,
    0x00cf, 0x00d0, 0x00d1, 0x00d2, 0x00d3, 0x00d4, 0x00d5, 0x00d6, 0x00d7,
    0x00d8, 0x00d9, 0x00da, 0x00db, 0x00dc, 0x00dd, 0x00de, 0x00df, 0x00e0,
    0x00e1, 0x00e2, 0x00e3, 0x00e4, 0x00e5, 0x00e6, 0x00e7, 0x00e8, 0x00e9,
    0x00ea, 0x00eb, 0x00ec, 0x00ed, 0x00ee, 0x00ef, 0x00f0, 0x00f1, 0x00f2,
    0x00f3, 0x00f4, 0x00f5, 0x00f6, 0x00f7, 0x00f8, 0x00f9, 0x00fa, 0x00fb,
    0x00fc, 0x00fd, 0x00fe, 0x00ff
];

// Core font names as a const array for better performance
const CORE_FONTS: &[&str] = &[
    "Courier-Bold",
    "Courier-BoldOblique",
    "Courier-Oblique",
    "Courier",
    "Helvetica-Bold",
    "Helvetica-BoldOblique",
    "Helvetica-Oblique",
    "Helvetica",
    "Symbol",
    "Times-Bold",
    "Times-BoldItalic",
    "Times-Italic",
    "Times-Roman",
    "ZapfDingbats",
];

/// Character code type for clarity
pub type CharCode = u32;

/// Helper functions for PDF string conversion
pub mod string_utils {
    use super::*;
    
    /// Convert PDF bytes to UTF-8 string
    pub fn pdf_to_utf8(s: &[u8]) -> PdfResult<String> {
        if s.len() >= 2 && s[0] == 0xfe && s[1] == 0xff {
            // UTF-16BE with BOM
            UTF_16BE.decode_without_bom_handling_and_without_replacement(&s[2..])
                .map(|s| s.to_string())
                .ok_or_else(|| PdfError::EncodingError("Invalid UTF-16BE".to_string()))
        } else {
            // Use PDF document encoding
            let utf16_bytes: Vec<u8> = s.iter()
                .flat_map(|&x| {
                    let k = PDF_DOC_ENCODING[x as usize];
                    [(k >> 8) as u8, k as u8]
                })
                .collect();
            
            UTF_16BE.decode_without_bom_handling_and_without_replacement(&utf16_bytes)
                .map(|s| s.to_string())
                .ok_or_else(|| PdfError::EncodingError("Invalid PDF encoding".to_string()))
        }
    }
    
    /// Convert to UTF-8 using specific encoding table
    pub fn to_utf8(encoding: &[u16], s: &[u8]) -> PdfResult<String> {
        if s.len() >= 2 && s[0] == 0xfe && s[1] == 0xff {
            // UTF-16BE with BOM
            UTF_16BE.decode_without_bom_handling_and_without_replacement(&s[2..])
                .map(|s| s.to_string())
                .ok_or_else(|| PdfError::EncodingError("Invalid UTF-16BE".to_string()))
        } else {
            let utf16_bytes: Vec<u8> = s.iter()
                .flat_map(|&x| {
                    let k = encoding.get(x as usize).copied().unwrap_or(0);
                    [(k >> 8) as u8, k as u8]
                })
                .collect();
            
            UTF_16BE.decode_without_bom_handling_and_without_replacement(&utf16_bytes)
                .map(|s| s.to_string())
                .ok_or_else(|| PdfError::EncodingError("Invalid encoding".to_string()))
        }
    }
}

/// PDF document helper functions
pub mod document_utils {
    use super::*;
    
    /// Get document info dictionary
    pub fn get_info(doc: &Document) -> Option<&Dictionary> {
        doc.trailer.get(b"Info").ok()
            .and_then(|obj| match obj {
                Object::Reference(id) => doc.get_object(*id).ok(),
                _ => None,
            })
            .and_then(|obj| match obj {
                Object::Dictionary(dict) => Some(dict),
                _ => None,
            })
    }
    
    /// Get document catalog
    pub fn get_catalog(doc: &Document) -> PdfResult<&Dictionary> {
        doc.trailer.get(b"Root")
            .map_err(|_| PdfError::MissingField("Root".to_string()))
            .and_then(|obj| match obj {
                Object::Reference(id) => doc.get_object(*id)
                    .map_err(PdfError::Parse),
                _ => Err(PdfError::InvalidStructure("Root must be a reference".to_string())),
            })
            .and_then(|obj| match obj {
                Object::Dictionary(dict) => Ok(dict),
                _ => Err(PdfError::InvalidStructure("Root must be a dictionary".to_string())),
            })
    }
    
    /// Get pages dictionary
    pub fn get_pages(doc: &Document) -> PdfResult<&Dictionary> {
        let catalog = get_catalog(doc)?;
        catalog.get(b"Pages")
            .map_err(|_| PdfError::MissingField("Pages".to_string()))
            .and_then(|obj| match obj {
                Object::Reference(id) => doc.get_object(*id)
                    .map_err(PdfError::Parse),
                _ => Err(PdfError::InvalidStructure("Pages must be a reference".to_string())),
            })
            .and_then(|obj| match obj {
                Object::Dictionary(dict) => Ok(dict),
                _ => Err(PdfError::InvalidStructure("Pages must be a dictionary".to_string())),
            })
    }
}

/// Object dereferencing and extraction utilities
pub mod object_utils {
    use super::*;
    
    /// Dereference an object if it's a reference
    pub fn maybe_deref<'a>(doc: &'a Document, obj: &'a Object) -> PdfResult<&'a Object> {
        match obj {
            Object::Reference(r) => doc.get_object(*r)
                .map_err(PdfError::Parse),
            _ => Ok(obj),
        }
    }
    
    /// Get object from dictionary with dereferencing
    pub fn maybe_get_obj<'a>(
        doc: &'a Document, 
        dict: &'a Dictionary, 
        key: &[u8]
    ) -> Option<&'a Object> {
        dict.get(key).ok()
            .and_then(|o| maybe_deref(doc, o).ok())
    }
    
    /// Convert object to number
    pub fn as_num(obj: &Object) -> PdfResult<f64> {
        match obj {
            Object::Integer(i) => Ok(*i as f64),
            Object::Real(f) => Ok((*f).into()),
            _ => Err(PdfError::InvalidStructure("Expected number".to_string())),
        }
    }
}

// Trait for converting from optional objects
trait FromOptObj<'a>: Sized {
    fn from_opt_obj(doc: &'a Document, obj: Option<&'a Object>, key: &[u8]) -> PdfResult<Self>;
}

// Trait for converting from objects
trait FromObj<'a>: Sized {
    fn from_obj(doc: &'a Document, obj: &'a Object) -> PdfResult<Self>;
}

// Implementations for common types
impl<'a, T: FromObj<'a>> FromOptObj<'a> for Option<T> {
    fn from_opt_obj(doc: &'a Document, obj: Option<&'a Object>, _key: &[u8]) -> PdfResult<Self> {
        match obj {
            Some(o) => T::from_obj(doc, o).map(Some),
            None => Ok(None),
        }
    }
}

impl<'a, T: FromObj<'a>> FromOptObj<'a> for T {
    fn from_opt_obj(doc: &'a Document, obj: Option<&'a Object>, key: &[u8]) -> PdfResult<Self> {
        obj.ok_or_else(|| PdfError::MissingField(String::from_utf8_lossy(key).to_string()))
            .and_then(|o| T::from_obj(doc, o))
    }
}

impl<'a, T: FromObj<'a>> FromObj<'a> for Vec<T> {
    fn from_obj(doc: &'a Document, obj: &'a Object) -> PdfResult<Self> {
        object_utils::maybe_deref(doc, obj)?
            .as_array()
            .map_err(|_| PdfError::InvalidStructure("Expected array".to_string()))
            .and_then(|arr| {
                arr.iter()
                    .map(|x| T::from_obj(doc, x))
                    .collect::<PdfResult<Vec<_>>>()
            })
    }
}

// Use const generics for fixed-size arrays
impl<'a, T: FromObj<'a>, const N: usize> FromObj<'a> for [T; N] {
    fn from_obj(doc: &'a Document, obj: &'a Object) -> PdfResult<Self> {
        let vec = Vec::<T>::from_obj(doc, obj)?;
        vec.try_into()
            .map_err(|v: Vec<T>| PdfError::InvalidStructure(
                format!("Expected array of length {}, got {}", N, v.len())
            ))
    }
}

impl<'a> FromObj<'a> for f64 {
    fn from_obj(_doc: &Document, obj: &Object) -> PdfResult<Self> {
        object_utils::as_num(obj)
    }
}

impl<'a> FromObj<'a> for i64 {
    fn from_obj(_doc: &Document, obj: &Object) -> PdfResult<Self> {
        match obj {
            Object::Integer(i) => Ok(*i),
            _ => Err(PdfError::InvalidStructure("Expected integer".to_string())),
        }
    }
}

impl<'a> FromObj<'a> for &'a Dictionary {
    fn from_obj(doc: &'a Document, obj: &'a Object) -> PdfResult<&'a Dictionary> {
        object_utils::maybe_deref(doc, obj)?
            .as_dict()
            .map_err(|_| PdfError::InvalidStructure("Expected dictionary".to_string()))
    }
}

impl<'a> FromObj<'a> for &'a Stream {
    fn from_obj(doc: &'a Document, obj: &'a Object) -> PdfResult<&'a Stream> {
        object_utils::maybe_deref(doc, obj)?
            .as_stream()
            .map_err(|_| PdfError::InvalidStructure("Expected stream".to_string()))
    }
}

impl<'a> FromObj<'a> for &'a Object {
    fn from_obj(doc: &'a Document, obj: &'a Object) -> PdfResult<&'a Object> {
        object_utils::maybe_deref(doc, obj)
    }
}

// Helper functions for getting values from dictionaries
fn get<'a, T: FromOptObj<'a>>(doc: &'a Document, dict: &'a Dictionary, key: &[u8]) -> PdfResult<T> {
    T::from_opt_obj(doc, dict.get(key).ok(), key)
}

fn maybe_get<'a, T: FromObj<'a>>(doc: &'a Document, dict: &'a Dictionary, key: &[u8]) -> Option<T> {
    object_utils::maybe_get_obj(doc, dict, key)
        .and_then(|o| T::from_obj(doc, o).ok())
}

fn get_name_string(doc: &Document, dict: &Dictionary, key: &[u8]) -> PdfResult<String> {
    dict.get(key)
        .map_err(|_| PdfError::MissingField(String::from_utf8_lossy(key).to_string()))
        .and_then(|o| object_utils::maybe_deref(doc, o))
        .and_then(|o| o.as_name()
            .map_err(|_| PdfError::InvalidStructure("Expected name".to_string())))
        .and_then(string_utils::pdf_to_utf8)
}

fn maybe_get_name_string(doc: &Document, dict: &Dictionary, key: &[u8]) -> Option<String> {
    object_utils::maybe_get_obj(doc, dict, key)
        .and_then(|n| n.as_name().ok())
        .and_then(|n| string_utils::pdf_to_utf8(n).ok())
}

fn maybe_get_name<'a>(doc: &'a Document, dict: &'a Dictionary, key: &[u8]) -> Option<&'a [u8]> {
    object_utils::maybe_get_obj(doc, dict, key)
        .and_then(|n| n.as_name().ok())
}

fn maybe_get_array<'a>(doc: &'a Document, dict: &'a Dictionary, key: &[u8]) -> Option<&'a Vec<Object>> {
    object_utils::maybe_get_obj(doc, dict, key)
        .and_then(|n| n.as_array().ok())
}

// Font trait and implementations
pub trait PdfFont: Debug + Send + Sync {
    fn get_width(&self, id: CharCode) -> f64;
    fn next_char(&self, iter: &mut Iter<u8>) -> Option<(CharCode, u8)>;
    fn decode_char(&self, char: CharCode) -> String;
    
    fn char_codes<'a>(&'a self, chars: &'a [u8]) -> PdfFontIter<'a> 
    where 
        Self: Sized 
    {
        PdfFontIter { 
            iter: chars.iter(), 
            font: self,
        }
    }
    
    fn decode(&self, chars: &[u8]) -> String {
        let mut result = String::new();
        let mut iter = chars.iter();
        while let Some((code, _)) = self.next_char(&mut iter) {
            result.push_str(&self.decode_char(code));
        }
        result
    }
}

pub struct PdfFontIter<'a> {
    iter: Iter<'a, u8>,
    font: &'a dyn PdfFont,
}

impl<'a> Iterator for PdfFontIter<'a> {
    type Item = (CharCode, u8);
    
    fn next(&mut self) -> Option<Self::Item> {
        self.font.next_char(&mut self.iter)
    }
}

// Font implementations
#[derive(Clone, Debug)]
pub struct PdfSimpleFont {
    base_name: String,
    encoding: Option<Vec<u16>>,
    unicode_map: Option<HashMap<CharCode, String>>,
    widths: HashMap<CharCode, f64>,
    missing_width: f64,
}

impl PdfSimpleFont {
    pub fn new(doc: &Document, font: &Dictionary) -> PdfResult<Self> {
        let base_name = get_name_string(doc, font, b"BaseFont")?;
        let subtype = get_name_string(doc, font, b"Subtype")?;
        
        debug!("Creating {} font: {}", subtype, base_name);
        
        let encoding = Self::load_encoding(doc, font, &base_name)?;
        // --- Begin: CFF/Type1C unicode map extraction ---
        let mut unicode_map = None;
        let descriptor: Option<&Dictionary> = get(doc, font, b"FontDescriptor")?;
        if let Some(desc) = descriptor {
            if let Some(Object::Stream(s)) = get::<Option<&Object>>(doc, desc, b"FontFile3")? {
                let subtype = get_name_string(doc, &s.dict, b"Subtype")?;
                if subtype == "Type1C" {
                    let contents = get_contents(s);
                    if let Some(cff) = Table::parse(&contents) {
                        let mut mapping = std::collections::HashMap::new();
                        let charset_table = cff.charset.get_table();
                        let encoding_table = cff.encoding.get_table();
                        for (i, (&cid, &sid)) in encoding_table.iter().zip(charset_table.iter()).enumerate() {
                            if let Some(name) = cff_parser::string_by_id(&cff, sid) {
                                let unicode = glyphnames::name_to_unicode(&name)
                                    .or_else(|| zapfglyphnames::zapfdigbats_names_to_unicode(&name));
                                if let Some(unicode) = unicode {
                                    if let Ok(s) = String::from_utf16(&[unicode]) {
                                        mapping.insert(cid as u32, s);
                                    }
                                }
                            }
                        }
                        // Merge with ToUnicode map if present
                        if let Some(mut to_unicode) = get_unicode_map(doc, font)? {
                            mapping.extend(to_unicode);
                        }
                        unicode_map = Some(mapping);
                    }
                }
            }
        }
        // --- End: CFF/Type1C unicode map extraction ---
        // If not set above, fallback to ToUnicode map
        let unicode_map = unicode_map.or_else(|| Self::load_unicode_map(doc, font).unwrap_or(None));
        let (widths, missing_width) = Self::load_widths(doc, font, &base_name, encoding.as_ref())?;
        
        Ok(Self {
            base_name: base_name,
            encoding,
            unicode_map,
            widths,
            missing_width,
        })
    }
    
    fn load_encoding(doc: &Document, font: &Dictionary, _base_name: &str) -> PdfResult<Option<Vec<u16>>> {
        let encoding_obj: Option<&Object> = get(doc, font, b"Encoding")?;
        
        match encoding_obj {
            Some(Object::Name(name)) => {
                Ok(Some(encoding_to_unicode_table(name)?))
            }
            Some(Object::Dictionary(dict)) => {
                let mut table = if let Some(base_encoding) = maybe_get_name(doc, dict, b"BaseEncoding") {
                    encoding_to_unicode_table(base_encoding)?
                } else {
                    Vec::from(PDF_DOC_ENCODING)
                };
                
                if let Some(differences) = maybe_get_array(doc, dict, b"Differences") {
                    Self::apply_encoding_differences(doc, &mut table, differences)?;
                }
                
                Ok(Some(table))
            }
            None => {
                // Handle Type1 and TrueType default encodings
                let descriptor: Option<&Dictionary> = get(doc, font, b"FontDescriptor")?;
                if let Some(desc) = descriptor {
                    if let Some(encoding) = Self::load_font_file_encoding(doc, desc, &get_name_string(doc, font, b"Subtype")?)? {
                        return Ok(Some(encoding));
                    }
                }
                
                // Default encoding for TrueType
                if get_name_string(doc, font, b"Subtype")? == "TrueType" {
                    Ok(Some(encoding_to_unicode_table(b"WinAnsiEncoding")?))
                } else {
                    Ok(None)
                }
            }
            _ => Err(PdfError::InvalidStructure("Invalid encoding type".to_string())),
        }
    }
    
    fn apply_encoding_differences(
        doc: &Document,
        table: &mut Vec<u16>,
        differences: &[Object],
    ) -> PdfResult<()> {
        let mut code = 0i64;
        
        for obj in differences {
            let obj = object_utils::maybe_deref(doc, obj)?;
            match obj {
                Object::Integer(i) => code = *i,
                Object::Name(n) => {
                    let name = string_utils::pdf_to_utf8(n)?;
                    if let Some(unicode) = glyphnames::name_to_unicode(&name) {
                        if code >= 0 && (code as usize) < table.len() {
                            table[code as usize] = unicode;
                        }
                    } else {
                        warn!("Unknown glyph name: {}", name);
                    }
                    code += 1;
                }
                _ => return Err(PdfError::InvalidStructure("Invalid differences entry".to_string())),
            }
        }
        
        Ok(())
    }
    
    fn load_font_file_encoding(doc: &Document, descriptor: &Dictionary, subtype: &str) -> PdfResult<Option<Vec<u16>>> {
        match subtype {
            "Type1" => {
                if let Some(Object::Stream(s)) = object_utils::maybe_get_obj(doc, descriptor, b"FontFile") {
                    let contents = get_contents(s);
                    if let Ok(encoding_map) = type1_encoding_parser::get_encoding_map(&contents) {
                        let mut table = Vec::from(PDF_DOC_ENCODING);
                        for (code, name) in encoding_map {
                            if let Ok(name_str) = string_utils::pdf_to_utf8(&name) {
                                if let Some(unicode) = glyphnames::name_to_unicode(&name_str) {
                                    if code >= 0 && (code as usize) < table.len() {
                                        table[code as usize] = unicode;
                                    }
                                }
                            }
                        }
                        return Ok(Some(table));
                    }
                }
            }
            "Type1C" => {
                if let Some(Object::Stream(s)) = get::<Option<&Object>>(doc, descriptor, b"FontFile3")? {
                    let subtype = get_name_string(doc, &s.dict, b"Subtype")?;
                    if subtype == "Type1C" {
                        let contents = get_contents(s);
                        if let Some(_cff) = Table::parse(&contents) {
                            // You can now use `_cff` to extract encoding/charset as needed
                            // For now, just return None as before, as this function returns Vec<u16>
                            // and CFF encoding is handled in PdfSimpleFont::new
                            return Ok(None);
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(None)
    }
    
    fn load_unicode_map(doc: &Document, font: &Dictionary) -> PdfResult<Option<HashMap<CharCode, String>>> {
        get_unicode_map(doc, font)
    }
    
    fn load_widths(
        doc: &Document,
        font: &Dictionary,
        base_name: &str,
        encoding: Option<&Vec<u16>>,
    ) -> PdfResult<(HashMap<CharCode, f64>, f64)> {
        let mut width_map = HashMap::new();
        let missing_width = get::<Option<f64>>(doc, font, b"MissingWidth")?.unwrap_or(0.0);
        
        // Try to load widths from font dictionary
        if let (Some(first_char), Some(_last_char), Some(widths)) = (
            maybe_get::<i64>(doc, font, b"FirstChar"),
            maybe_get::<i64>(doc, font, b"LastChar"),
            maybe_get::<Vec<f64>>(doc, font, b"Widths"),
        ) {
            for (i, &width) in widths.iter().enumerate() {
                width_map.insert((first_char + i as i64) as CharCode, width);
            }
        } else if is_core_font(base_name) {
            // Load core font metrics
            Self::load_core_font_widths(&mut width_map, base_name, encoding)?;
        } else {
            warn!("No widths found for non-core font: {}", base_name);
        }
        
        Ok((width_map, missing_width))
    }
    
    fn load_core_font_widths(
        width_map: &mut HashMap<CharCode, f64>,
        base_name: &str,
        encoding: Option<&Vec<u16>>,
    ) -> PdfResult<()> {
        for font_metrics in core_fonts::metrics() {
            if font_metrics.0 == base_name {
                if let Some(encoding) = encoding {
                    for w in font_metrics.2 {
                        let c = glyphnames::name_to_unicode(w.2).unwrap_or(0);
                        for (i, &enc_char) in encoding.iter().enumerate() {
                            if enc_char == c {
                                width_map.insert(i as CharCode, w.1);
                            }
                        }
                    }
                } else {
                    for w in font_metrics.2 {
                        if w.0 >= 0 {
                            width_map.insert(w.0 as CharCode, w.1);
                        }
                    }
                }
                break;
            }
        }
        Ok(())
    }
}

impl PdfFont for PdfSimpleFont {
    fn get_width(&self, id: CharCode) -> f64 {
        self.widths.get(&id).copied().unwrap_or_else(|| {
            debug!("Missing width for char {} in font {}, using missing_width", id, self.base_name);
            self.missing_width
        })
    }
    
    fn next_char(&self, iter: &mut Iter<u8>) -> Option<(CharCode, u8)> {
        iter.next().map(|&b| (b as CharCode, 1))
    }
    
    fn decode_char(&self, char: CharCode) -> String {
        if let Some(unicode_map) = &self.unicode_map {
            if let Some(s) = unicode_map.get(&char) {
                return s.clone();
            }
            warn!("Missing char {} in unicode map for font {}", char, self.base_name);
        }
        
        let encoding = self.encoding.as_deref().unwrap_or(PDF_DOC_ENCODING);
        let byte = (char & 0xFF) as u8;
        string_utils::to_utf8(encoding, &[byte]).unwrap_or_else(|_| {
            warn!("Failed to decode char {} in font {}", char, self.base_name);
            String::new()
        })
    }
}

#[derive(Clone, Debug)]
pub struct PdfType3Font {
    encoding: Option<Vec<u16>>,
    unicode_map: Option<HashMap<CharCode, String>>,
    widths: HashMap<CharCode, f64>,
}

impl PdfType3Font {
    pub fn new(doc: &Document, font: &Dictionary) -> PdfResult<Self> {
        let encoding = Self::load_encoding(doc, font)?;
        let unicode_map = get_unicode_map(doc, font)?;
        let widths = Self::load_widths(doc, font)?;
        
        Ok(Self {
            encoding,
            unicode_map,
            widths,
        })
    }
    
    fn load_encoding(doc: &Document, font: &Dictionary) -> PdfResult<Option<Vec<u16>>> {
        let encoding_obj: Option<&Object> = get(doc, font, b"Encoding")?;
        
        match encoding_obj {
            Some(Object::Name(name)) => Ok(Some(encoding_to_unicode_table(name)?)),
            Some(Object::Dictionary(dict)) => {
                let mut table = if let Some(base_encoding) = maybe_get_name(doc, dict, b"BaseEncoding") {
                    encoding_to_unicode_table(base_encoding)?
                } else {
                    Vec::from(PDF_DOC_ENCODING)
                };
                
                if let Some(differences) = maybe_get_array(doc, dict, b"Differences") {
                    PdfSimpleFont::apply_encoding_differences(doc, &mut table, differences)?;
                }
                
                Ok(Some(table))
            }
            _ => Err(PdfError::InvalidStructure("Invalid encoding type".to_string())),
        }
    }
    
    fn load_widths(doc: &Document, font: &Dictionary) -> PdfResult<HashMap<CharCode, f64>> {
        let first_char: i64 = get(doc, font, b"FirstChar")?;
        let last_char: i64 = get(doc, font, b"LastChar")?;
        let widths: Vec<f64> = get(doc, font, b"Widths")?;
        
        let mut width_map = HashMap::new();
        for (i, &width) in widths.iter().enumerate() {
            width_map.insert((first_char + i as i64) as CharCode, width);
        }
        
        if width_map.len() != (last_char - first_char + 1) as usize {
            return Err(PdfError::InvalidStructure("Width array size mismatch".to_string()));
        }
        
        Ok(width_map)
    }
}

impl PdfFont for PdfType3Font {
    fn get_width(&self, id: CharCode) -> f64 {
        self.widths.get(&id).copied().unwrap_or_else(|| {
            error!("Missing width for char {} in Type3 font", id);
            0.0
        })
    }
    
    fn next_char(&self, iter: &mut Iter<u8>) -> Option<(CharCode, u8)> {
        iter.next().map(|&b| (b as CharCode, 1))
    }
    
    fn decode_char(&self, char: CharCode) -> String {
        if let Some(unicode_map) = &self.unicode_map {
            if let Some(s) = unicode_map.get(&char) {
                return s.clone();
            }
        }
        
        let encoding = self.encoding.as_deref().unwrap_or(PDF_DOC_ENCODING);
        let byte = (char & 0xFF) as u8;
        string_utils::to_utf8(encoding, &[byte]).unwrap_or_else(|_| String::new())
    }
}

// Wrapper for ByteMapping to make it cloneable
#[derive(Debug)]
pub struct CIDFontEncoding {
    codespace: Vec<CodeRange>,
    cid: Vec<CIDRange>,
}

impl Clone for CIDFontEncoding {
    fn clone(&self) -> Self {
        Self {
            codespace: self.codespace.iter().map(|r| CodeRange {
                width: r.width,
                start: r.start,
                end: r.end,
            }).collect(),
            cid: self.cid.iter().map(|r| CIDRange {
                src_code_lo: r.src_code_lo,
                src_code_hi: r.src_code_hi,
                dst_CID_lo: r.dst_CID_lo,
            }).collect(),
        }
    }
}

impl From<ByteMapping> for CIDFontEncoding {
    fn from(mapping: ByteMapping) -> Self {
        Self {
            codespace: mapping.codespace,
            cid: mapping.cid,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PdfCIDFont {
    encoding: CIDFontEncoding,
    to_unicode: Option<HashMap<CharCode, String>>,
    widths: HashMap<CharCode, f64>,
    default_width: f64,
}

impl PdfCIDFont {
    pub fn new(doc: &Document, font: &Dictionary) -> PdfResult<Self> {
        let base_name = get_name_string(doc, font, b"BaseFont")?;
        debug!("Creating CID font: {}", base_name);
        
        let descendants = maybe_get_array(doc, font, b"DescendantFonts")
            .ok_or_else(|| PdfError::MissingField("DescendantFonts".to_string()))?;
        
        let cid_dict = object_utils::maybe_deref(doc, &descendants[0])?
            .as_dict()
            .map_err(|_| PdfError::InvalidStructure("Invalid CID dictionary".to_string()))?;
        
        let encoding = Self::load_encoding(doc, font)?;
        let to_unicode = get_unicode_map(doc, font)?;
        let (widths, default_width) = Self::load_widths(doc, cid_dict)?;
        
        Ok(Self {
            encoding: encoding.into(),
            to_unicode,
            widths,
            default_width,
        })
    }
    
    fn load_encoding(doc: &Document, font: &Dictionary) -> PdfResult<ByteMapping> {
        let encoding_obj = object_utils::maybe_get_obj(doc, font, b"Encoding")
            .ok_or_else(|| PdfError::MissingField("Encoding".to_string()))?;
        
        match encoding_obj {
            Object::Name(name) => {
                let name_str = string_utils::pdf_to_utf8(name)?;
                match name_str.as_str() {
                    "Identity-H" | "Identity-V" => Ok(ByteMapping {
                        codespace: vec![CodeRange { width: 2, start: 0, end: 0xffff }],
                        cid: vec![CIDRange { 
                            src_code_lo: 0, 
                            src_code_hi: 0xffff, 
                            dst_CID_lo: 0 
                        }],
                    }),
                    _ => Err(PdfError::InvalidStructure(format!("Unsupported encoding: {}", name_str))),
                }
            }
            Object::Stream(stream) => {
                let contents = get_contents(stream);
                adobe_cmap_parser::get_byte_mapping(&contents)
                    .map_err(|_| PdfError::InvalidStructure("Invalid CMap".to_string()))
            }
            _ => Err(PdfError::InvalidStructure("Invalid encoding type".to_string())),
        }
    }
    
    fn load_widths(doc: &Document, cid_dict: &Dictionary) -> PdfResult<(HashMap<CharCode, f64>, f64)> {
        let default_width = get::<Option<i64>>(doc, cid_dict, b"DW")?
            .unwrap_or(1000) as f64;
        
        let mut widths = HashMap::new();
        
        if let Some(w_array) = get::<Option<Vec<&Object>>>(doc, cid_dict, b"W")? {
            let mut i = 0;
            while i < w_array.len() {
                if i + 1 < w_array.len() {
                    if let Ok(array) = w_array[i + 1].as_array() {
                        // Format: c [w1 w2 ...]
                        let cid = w_array[i].as_i64()
                            .map_err(|_| PdfError::InvalidStructure("Invalid CID".to_string()))?;
                        
                        for (j, w) in array.iter().enumerate() {
                            widths.insert((cid + j as i64) as CharCode, object_utils::as_num(w)?);
                        }
                        i += 2;
                    } else if i + 2 < w_array.len() {
                        // Format: c_first c_last w
                        let c_first = w_array[i].as_i64()
                            .map_err(|_| PdfError::InvalidStructure("Invalid CID".to_string()))?;
                        let c_last = w_array[i + 1].as_i64()
                            .map_err(|_| PdfError::InvalidStructure("Invalid CID".to_string()))?;
                        let width = object_utils::as_num(w_array[i + 2])?;
                        
                        for cid in c_first..=c_last {
                            widths.insert(cid as CharCode, width);
                        }
                        i += 3;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        
        Ok((widths, default_width))
    }
}

impl PdfFont for PdfCIDFont {
    fn get_width(&self, id: CharCode) -> f64 {
        self.widths.get(&id).copied().unwrap_or(self.default_width)
    }
    
    fn next_char(&self, iter: &mut Iter<u8>) -> Option<(CharCode, u8)> {
        let first = *iter.next()?;
        let mut code = first as u32;
        
        // Check codespace ranges to determine character width
        for range in &self.encoding.codespace {
            if code >= range.start && code <= range.end && range.width == 1 {
                // Map through CID ranges
                for cid_range in &self.encoding.cid {
                    if code >= cid_range.src_code_lo && code <= cid_range.src_code_hi {
                        return Some((code - cid_range.src_code_lo + cid_range.dst_CID_lo, 1));
                    }
                }
            }
        }
        
        // Try multi-byte sequences
        for bytes in 2..=4 {
            if let Some(&next_byte) = iter.as_slice().get(bytes - 2) {
                code = (code << 8) | (next_byte as u32);
                
                for range in &self.encoding.codespace {
                    if code >= range.start && code <= range.end && range.width == bytes as u32 {
                        // Consume the additional bytes
                        for _ in 1..bytes {
                            iter.next();
                        }
                        
                        // Map through CID ranges
                        for cid_range in &self.encoding.cid {
                            if code >= cid_range.src_code_lo && code <= cid_range.src_code_hi {
                                return Some((
                                    code - cid_range.src_code_lo + cid_range.dst_CID_lo,
                                    bytes as u8
                                ));
                            }
                        }
                    }
                }
            } else {
                break;
            }
        }
        
        None
    }
    
    fn decode_char(&self, char: CharCode) -> String {
        self.to_unicode.as_ref()
            .and_then(|map| map.get(&char))
            .cloned()
            .unwrap_or_else(|| {
                debug!("Unknown character {} in CID font", char);
                String::new()
            })
    }
}

// Font factory function
pub fn make_font(doc: &Document, font: &Dictionary) -> PdfResult<Arc<dyn PdfFont>> {
    let subtype = get_name_string(doc, font, b"Subtype")?;
    
    match subtype.as_str() {
        "Type0" => Ok(Arc::new(PdfCIDFont::new(doc, font)?)),
        "Type3" => Ok(Arc::new(PdfType3Font::new(doc, font)?)),
        _ => Ok(Arc::new(PdfSimpleFont::new(doc, font)?)),
    }
}

// Helper functions
fn is_core_font(name: &str) -> bool {
    CORE_FONTS.contains(&name)
}

fn encoding_to_unicode_table(name: &[u8]) -> PdfResult<Vec<u16>> {
    let encoding = match name {
        b"MacRomanEncoding" => encodings::MAC_ROMAN_ENCODING,
        b"MacExpertEncoding" => encodings::MAC_EXPERT_ENCODING,
        b"WinAnsiEncoding" => encodings::WIN_ANSI_ENCODING,
        _ => return Err(PdfError::InvalidStructure(
            format!("Unknown encoding: {:?}", string_utils::pdf_to_utf8(name)?)
        )),
    };
    
    Ok(encoding.iter()
        .map(|&opt| opt.and_then(glyphnames::name_to_unicode).unwrap_or(0))
        .collect())
}

fn get_unicode_map(doc: &Document, font: &Dictionary) -> PdfResult<Option<HashMap<CharCode, String>>> {
    let to_unicode = object_utils::maybe_get_obj(doc, font, b"ToUnicode");
    
    match to_unicode {
        Some(Object::Stream(stream)) => {
            let contents = get_contents(stream);
            let cmap = adobe_cmap_parser::get_unicode_map(&contents)
                .map_err(|_| PdfError::InvalidStructure("Invalid ToUnicode CMap".to_string()))?;
            
            let mut unicode_map = HashMap::new();
            
            for (&k, v) in cmap.iter() {
                // Convert UTF-16BE bytes to string
                let utf16_values: Vec<u16> = v.chunks_exact(2)
                    .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
                    .collect();
                
                // Skip surrogate pairs that are invalid
                if utf16_values.len() == 1 && (0xD800..=0xDFFF).contains(&utf16_values[0]) {
                    continue;
                }
                
                match String::from_utf16(&utf16_values) {
                    Ok(s) => { unicode_map.insert(k, s); }
                    Err(_) => { warn!("Invalid UTF-16 sequence for character {}", k); }
                }
            }
            
            Ok(Some(unicode_map))
        }
        Some(Object::Name(name)) => {
            let name_str = string_utils::pdf_to_utf8(name)?;
            if name_str != "Identity-H" {
                warn!("Unsupported ToUnicode name: {}", name_str);
            }
            Ok(None)
        }
        None => Ok(None),
        _ => Err(PdfError::InvalidStructure("Invalid ToUnicode type".to_string())),
    }
}

fn get_contents(stream: &Stream) -> Vec<u8> {
    stream.decompressed_content()
        .unwrap_or_else(|_| stream.content.clone())
}

// Add missing type1_encoding_parser module
mod type1_encoding_parser {
    use std::collections::HashMap;
    
    pub fn get_encoding_map(data: &[u8]) -> Result<HashMap<i64, Vec<u8>>, &'static str> {
        let _ = data;
        // Simplified implementation - in real code this would parse Type1 font encoding
        Ok(HashMap::new())
    }
}

// Output device trait and implementations
pub trait OutputDev {
    fn begin_page(&mut self, page_num: u32, media_box: &MediaBox, art_box: Option<(f64, f64, f64, f64)>) -> PdfResult<()>;
    fn end_page(&mut self) -> PdfResult<()>;
    fn output_character(&mut self, trm: &PdfTransform, width: f64, spacing: f64, font_size: f64, char: &str) -> PdfResult<()>;
    fn begin_word(&mut self) -> PdfResult<()>;
    fn end_word(&mut self) -> PdfResult<()>;
    fn end_line(&mut self) -> PdfResult<()>;
    fn stroke(&mut self, _ctm: &PdfTransform, _colorspace: &ColorSpace, _color: &[f64], _path: &Path) -> PdfResult<()> { Ok(()) }
    fn fill(&mut self, _ctm: &PdfTransform, _colorspace: &ColorSpace, _color: &[f64], _path: &Path) -> PdfResult<()> { Ok(()) }
}

// MediaBox type
#[derive(Debug, Clone, Copy)]
pub struct MediaBox {
    pub llx: f64,
    pub lly: f64,
    pub urx: f64,
    pub ury: f64,
}

// Path operations
#[derive(Debug)]
pub enum PathOp {
    MoveTo(f64, f64),
    LineTo(f64, f64),
    CurveTo(f64, f64, f64, f64, f64, f64),
    Rect(f64, f64, f64, f64),
    Close,
}

#[derive(Debug)]
pub struct Path {
    pub ops: Vec<PathOp>,
}

impl Path {
    fn new() -> Path {
        Path { ops: Vec::new() }
    }
    
    fn current_point(&self) -> (f64, f64) {
        match self.ops.last() {
            Some(PathOp::MoveTo(x, y)) => (*x, *y),
            Some(PathOp::LineTo(x, y)) => (*x, *y),
            Some(PathOp::CurveTo(_, _, _, _, x, y)) => (*x, *y),
            _ => panic!("No current point"),
        }
    }
}

// Color space types
#[derive(Clone, Debug)]
pub struct CalGray {
    white_point: [f64; 3],
    black_point: Option<[f64; 3]>,
    gamma: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct CalRGB {
    white_point: [f64; 3],
    black_point: Option<[f64; 3]>,
    gamma: Option<[f64; 3]>,
    matrix: Option<Vec<f64>>,
}

#[derive(Clone, Debug)]
pub struct Lab {
    white_point: [f64; 3],
    black_point: Option<[f64; 3]>,
    range: Option<[f64; 4]>,
}

#[derive(Clone, Debug)]
pub enum AlternateColorSpace {
    DeviceGray,
    DeviceRGB,
    DeviceCMYK,
    CalRGB(CalRGB),
    CalGray(CalGray),
    Lab(Lab),
    ICCBased(Vec<u8>),
}

#[derive(Clone)]
pub struct Separation {
    name: String,
    alternate_space: AlternateColorSpace,
    tint_transform: Box<Function>,
}

#[derive(Clone)]
pub enum ColorSpace {
    DeviceGray,
    DeviceRGB,
    DeviceCMYK,
    DeviceN,
    Pattern,
    CalRGB(CalRGB),
    CalGray(CalGray),
    Lab(Lab),
    Separation(Separation),
    ICCBased(Vec<u8>),
}

// Function types
#[derive(Clone, Debug)]
struct Type0Func {
    domain: Vec<f64>,
    range: Vec<f64>,
    contents: Vec<u8>,
    size: Vec<i64>,
    bits_per_sample: i64,
    encode: Vec<f64>,
    decode: Vec<f64>,
}

#[derive(Clone, Debug)]
struct Type2Func {
    c0: Option<Vec<f64>>,
    c1: Option<Vec<f64>>,
    n: f64,
}

#[derive(Clone, Debug)]
enum Function {
    Type0(Type0Func),
    Type2(Type2Func),
    Type3,
    Type4(Vec<u8>),
}

impl Function {
    fn new(doc: &Document, obj: &Object) -> PdfResult<Function> {
        let dict = match obj {
            Object::Dictionary(dict) => dict,
            Object::Stream(stream) => &stream.dict,
            _ => return Err(PdfError::InvalidStructure("Function must be dict or stream".to_string())),
        };
        
        let function_type: i64 = get(doc, dict, b"FunctionType")?;
        
        match function_type {
            0 => {
                let stream = match obj {
                    Object::Stream(stream) => stream,
                    _ => return Err(PdfError::InvalidStructure("Type 0 function must be stream".to_string())),
                };
                let range: Vec<f64> = get(doc, dict, b"Range")?;
                let domain: Vec<f64> = get(doc, dict, b"Domain")?;
                let contents = get_contents(stream);
                let size: Vec<i64> = get(doc, dict, b"Size")?;
                let bits_per_sample = get(doc, dict, b"BitsPerSample")?;
                
                let encode = get::<Option<Vec<f64>>>(doc, dict, b"Encode")?
                    .unwrap_or_else(|| {
                        let mut default = Vec::new();
                        for i in &size {
                            default.push(0.);
                            default.push((i - 1) as f64);
                        }
                        default
                    });
                
                let decode = get::<Option<Vec<f64>>>(doc, dict, b"Decode")?
                    .unwrap_or_else(|| range.clone());
                
                Ok(Function::Type0(Type0Func {
                    domain,
                    range,
                    size,
                    contents,
                    bits_per_sample,
                    encode,
                    decode,
                }))
            }
            2 => {
                let c0 = get::<Option<Vec<f64>>>(doc, dict, b"C0")?;
                let c1 = get::<Option<Vec<f64>>>(doc, dict, b"C1")?;
                let n = get::<f64>(doc, dict, b"N")?;
                Ok(Function::Type2(Type2Func { c0, c1, n }))
            }
            3 => Ok(Function::Type3),
            4 => {
                let contents = match obj {
                    Object::Stream(stream) => {
                        let contents = get_contents(stream);
                        warn!("Unhandled type-4 function");
                        contents
                    }
                    _ => return Err(PdfError::InvalidStructure("Type 4 function must be stream".to_string())),
                };
                Ok(Function::Type4(contents))
            }
            _ => Err(PdfError::InvalidStructure(format!("Unknown function type {}", function_type))),
        }
    }
}

// PlainTextOutput implementation
pub struct PlainTextOutput<W: std::io::Write> {
    writer: W,
    last_end: f64,
    last_y: f64,
    first_char: bool,
    flip_ctm: PdfTransform,
}

impl<W: std::io::Write> PlainTextOutput<W> {
    pub fn new(writer: W) -> PlainTextOutput<W> {
        PlainTextOutput {
            writer,
            last_end: 100000.,
            first_char: false,
            last_y: 0.,
            flip_ctm: Transform2D::identity(),
        }
    }
}

impl<W: std::io::Write> OutputDev for PlainTextOutput<W> {
    fn begin_page(&mut self, _page_num: u32, media_box: &MediaBox, _: Option<(f64, f64, f64, f64)>) -> PdfResult<()> {
        self.flip_ctm = Transform2D::new(1., 0., 0., -1., 0., media_box.ury - media_box.lly);
        Ok(())
    }
    
    fn end_page(&mut self) -> PdfResult<()> {
        Ok(())
    }
    
    fn output_character(&mut self, trm: &PdfTransform, width: f64, _spacing: f64, font_size: f64, char: &str) -> PdfResult<()> {
        let position = trm.then(&self.flip_ctm);
        let transformed_font_size_vec = trm.transform_vector(vec2(font_size, font_size));
        let transformed_font_size = (transformed_font_size_vec.x * transformed_font_size_vec.y).sqrt();
        let (x, y) = (position.m31, position.m32);
        
        if self.first_char {
            if (y - self.last_y).abs() > transformed_font_size * 1.5 {
                writeln!(self.writer)?;
            }
            
            if x < self.last_end && (y - self.last_y).abs() > transformed_font_size * 0.5 {
                writeln!(self.writer)?;
            }
            
            if x > self.last_end + transformed_font_size * 0.1 {
                write!(self.writer, " ")?;
            }
        }
        
        write!(self.writer, "{}", char)?;
        self.first_char = false;
        self.last_y = y;
        self.last_end = x + width * transformed_font_size;
        Ok(())
    }
    
    fn begin_word(&mut self) -> PdfResult<()> {
        self.first_char = true;
        Ok(())
    }
    
    fn end_word(&mut self) -> PdfResult<()> { Ok(()) }
    fn end_line(&mut self) -> PdfResult<()> { Ok(()) }
}

// HTMLOutput implementation
pub struct HTMLOutput<W: std::io::Write> {
    file: W,
    flip_ctm: PdfTransform,
    last_ctm: PdfTransform,
    buf_ctm: PdfTransform,
    buf_font_size: f64,
    buf: String,
}

impl<W: std::io::Write> HTMLOutput<W> {
    pub fn new(file: W) -> HTMLOutput<W> {
        HTMLOutput {
            file,
            flip_ctm: Transform2D::identity(),
            last_ctm: Transform2D::identity(),
            buf_ctm: Transform2D::identity(),
            buf: String::new(),
            buf_font_size: 0.,
        }
    }
    
    fn flush_string(&mut self) -> PdfResult<()> {
        if !self.buf.is_empty() {
            let position = self.buf_ctm.then(&self.flip_ctm);
            let transformed_font_size_vec = self.buf_ctm.transform_vector(vec2(self.buf_font_size, self.buf_font_size));
            let transformed_font_size = (transformed_font_size_vec.x * transformed_font_size_vec.y).sqrt();
            let (x, y) = (position.m31, position.m32);
            
            writeln!(self.file, "<div style='position: absolute; left: {}px; top: {}px; font-size: {}px'>{}</div>",
                   x, y, transformed_font_size, insert_nbsp(&self.buf))?;
            self.buf.clear();
        }
        Ok(())
    }
}

fn insert_nbsp(input: &str) -> String {
    let mut result = String::new();
    let mut word_end = false;
    let mut chars = input.chars().peekable();
    
    while let Some(c) = chars.next() {
        if c == ' ' {
            if !word_end || chars.peek().filter(|x| **x != ' ').is_none() {
                result += "&nbsp;";
            } else {
                result += " ";
            }
            word_end = false;
        } else {
            word_end = true;
            result.push(c);
        }
    }
    result
}

impl<W: std::io::Write> OutputDev for HTMLOutput<W> {
    fn begin_page(&mut self, page_num: u32, media_box: &MediaBox, _: Option<(f64, f64, f64, f64)>) -> PdfResult<()> {
        write!(self.file, "<meta charset='utf-8' />")?;
        write!(self.file, "<!-- page {} -->", page_num)?;
        write!(self.file, "<div id='page{}' style='position: relative; height: {}px; width: {}px; border: 1px black solid'>",
               page_num, media_box.ury - media_box.lly, media_box.urx - media_box.llx)?;
        self.flip_ctm = Transform2D::new(1., 0., 0., -1., 0., media_box.ury - media_box.lly);
        Ok(())
    }
    
    fn end_page(&mut self) -> PdfResult<()> {
        self.flush_string()?;
        self.buf.clear();
        self.last_ctm = Transform2D::identity();
        write!(self.file, "</div>")?;
        Ok(())
    }
    
    fn output_character(&mut self, trm: &PdfTransform, width: f64, spacing: f64, font_size: f64, char: &str) -> PdfResult<()> {
        if trm.approx_eq(&self.last_ctm) {
            self.buf += char;
        } else {
            self.flush_string()?;
            self.buf = char.to_owned();
            self.buf_font_size = font_size;
            self.buf_ctm = *trm;
        }
        self.last_ctm = trm.then(&Transform2D::translation(width * font_size + spacing, 0.));
        Ok(())
    }
    
    fn begin_word(&mut self) -> PdfResult<()> { Ok(()) }
    fn end_word(&mut self) -> PdfResult<()> { Ok(()) }
    fn end_line(&mut self) -> PdfResult<()> { Ok(()) }
}

// SVGOutput implementation
pub struct SVGOutput<W: std::io::Write> {
    file: W,
}

impl<W: std::io::Write> SVGOutput<W> {
    pub fn new(file: W) -> SVGOutput<W> {
        SVGOutput { file }
    }
}

impl<W: std::io::Write> OutputDev for SVGOutput<W> {
    fn begin_page(&mut self, _page_num: u32, media_box: &MediaBox, art_box: Option<(f64, f64, f64, f64)>) -> PdfResult<()> {
        let ver = 1.1;
        writeln!(self.file, "<?xml version=\"1.0\" encoding=\"UTF-8\" ?>")?;
        write!(self.file, r#"<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">"#)?;
        
        if let Some(art_box) = art_box {
            let width = art_box.2 - art_box.0;
            let height = art_box.3 - art_box.1;
            let y = media_box.ury - art_box.1 - height;
            write!(self.file, "<svg width=\"{}\" height=\"{}\" xmlns=\"http://www.w3.org/2000/svg\" version=\"{}\" viewBox='{} {} {} {}'>",
                   width, height, ver, art_box.0, y, width, height)?;
        } else {
            let width = media_box.urx - media_box.llx;
            let height = media_box.ury - media_box.lly;
            write!(self.file, "<svg width=\"{}\" height=\"{}\" xmlns=\"http://www.w3.org/2000/svg\" version=\"{}\" viewBox='{} {} {} {}'>",
                   width, height, ver, media_box.llx, media_box.lly, width, height)?;
        }
        writeln!(self.file)?;
        
        let ctm: PdfTransform = Transform2D::scale(1., -1.).then_translate(vec2(0., media_box.ury));
        writeln!(self.file, "<g transform='matrix({}, {}, {}, {}, {}, {})'>",
               ctm.m11, ctm.m12, ctm.m21, ctm.m22, ctm.m31, ctm.m32)?;
        Ok(())
    }
    
    fn end_page(&mut self) -> PdfResult<()> {
        writeln!(self.file, "</g>")?;
        write!(self.file, "</svg>")?;
        Ok(())
    }
    
    fn output_character(&mut self, _trm: &PdfTransform, _width: f64, _spacing: f64, _font_size: f64, _char: &str) -> PdfResult<()> {
        Ok(())
    }
    
    fn begin_word(&mut self) -> PdfResult<()> { Ok(()) }
    fn end_word(&mut self) -> PdfResult<()> { Ok(()) }
    fn end_line(&mut self) -> PdfResult<()> { Ok(()) }
    
    fn fill(&mut self, ctm: &PdfTransform, _colorspace: &ColorSpace, _color: &[f64], path: &Path) -> PdfResult<()> {
        write!(self.file, "<g transform='matrix({}, {}, {}, {}, {}, {})'>",
               ctm.m11, ctm.m12, ctm.m21, ctm.m22, ctm.m31, ctm.m32)?;
        
        let mut d = Vec::new();
        for op in &path.ops {
            match op {
                PathOp::MoveTo(x, y) => d.push(format!("M{} {}", x, y)),
                PathOp::LineTo(x, y) => d.push(format!("L{} {}", x, y)),
                PathOp::CurveTo(x1, y1, x2, y2, x, y) => d.push(format!("C{} {} {} {} {} {}", x1, y1, x2, y2, x, y)),
                PathOp::Close => d.push("Z".to_string()),
                PathOp::Rect(x, y, width, height) => {
                    d.push(format!("M{} {}", x, y));
                    d.push(format!("L{} {}", x + width, y));
                    d.push(format!("L{} {}", x + width, y + height));
                    d.push(format!("L{} {}", x, y + height));
                    d.push("Z".to_string());
                }
            }
        }
        
        write!(self.file, "<path d='{}' />", d.join(" "))?;
        writeln!(self.file, "</g>")?;
        Ok(())
    }
}

// Text extraction functions
pub fn extract_text<P: AsRef<std::path::Path>>(path: P) -> PdfResult<String> {
    let mut s = Vec::new();
    {
        let mut output = PlainTextOutput::new(&mut s);
        let mut doc = Document::load(path)?;
        maybe_decrypt(&mut doc)?;
        output_doc(&doc, &mut output)?;
    }
    String::from_utf8(s).map_err(|_| PdfError::EncodingError("Invalid UTF-8".to_string()))
}

fn maybe_decrypt(doc: &mut Document) -> PdfResult<()> {
    if !doc.is_encrypted() {
        return Ok(());
    }
    
    if let Err(e) = doc.decrypt("") {
        if let Error::Decryption(DecryptionError::IncorrectPassword) = e {
            error!("Encrypted documents must be decrypted with a password");
        }
        return Err(PdfError::Parse(e));
    }
    
    Ok(())
}

pub fn extract_text_encrypted<P: AsRef<std::path::Path>>(
    path: P,
    password: &str,
) -> PdfResult<String> {
    let mut s = Vec::new();
    {
        let mut output = PlainTextOutput::new(&mut s);
        let mut doc = Document::load(path)?;
        output_doc_encrypted(&mut doc, &mut output, password)?;
    }
    String::from_utf8(s).map_err(|_| PdfError::EncodingError("Invalid UTF-8".to_string()))
}

pub fn extract_text_from_mem(buffer: &[u8]) -> PdfResult<String> {
    let mut s = Vec::new();
    {
        let mut output = PlainTextOutput::new(&mut s);
        let mut doc = Document::load_mem(buffer)?;
        maybe_decrypt(&mut doc)?;
        output_doc(&doc, &mut output)?;
    }
    String::from_utf8(s).map_err(|_| PdfError::EncodingError("Invalid UTF-8".to_string()))
}

pub fn extract_text_from_mem_encrypted(
    buffer: &[u8],
    password: &str,
) -> PdfResult<String> {
    let mut s = Vec::new();
    {
        let mut output = PlainTextOutput::new(&mut s);
        let mut doc = Document::load_mem(buffer)?;
        output_doc_encrypted(&mut doc, &mut output, password)?;
    }
    String::from_utf8(s).map_err(|_| PdfError::EncodingError("Invalid UTF-8".to_string()))
}

pub fn extract_text_by_pages<P: AsRef<std::path::Path>>(path: P) -> PdfResult<Vec<String>> {
    let mut v = Vec::new();
    {
        let mut doc = Document::load(path)?;
        maybe_decrypt(&mut doc)?;
        let mut page_num = 1;
        while let Ok(content) = extract_text_by_page(&doc, page_num) {
            v.push(content);
            page_num += 1;
        }
    }
    Ok(v)
}

pub fn extract_text_by_pages_encrypted<P: AsRef<std::path::Path>>(
    path: P,
    password: &str,
) -> PdfResult<Vec<String>> {
    let mut v = Vec::new();
    {
        let mut doc = Document::load(path)?;
        doc.decrypt(password)?;
        let mut page_num = 1;
        while let Ok(content) = extract_text_by_page(&doc, page_num) {
            v.push(content);
            page_num += 1;
        }
    }
    Ok(v)
}

pub fn extract_text_from_mem_by_pages(buffer: &[u8]) -> PdfResult<Vec<String>> {
    let mut v = Vec::new();
    {
        let mut doc = Document::load_mem(buffer)?;
        maybe_decrypt(&mut doc)?;
        let mut page_num = 1;
        while let Ok(content) = extract_text_by_page(&doc, page_num) {
            v.push(content);
            page_num += 1;
        }
    }
    Ok(v)
}

pub fn extract_text_from_mem_by_pages_encrypted(
    buffer: &[u8],
    password: &str,
) -> PdfResult<Vec<String>> {
    let mut v = Vec::new();
    {
        let mut doc = Document::load_mem(buffer)?;
        doc.decrypt(password)?;
        let mut page_num = 1;
        while let Ok(content) = extract_text_by_page(&doc, page_num) {
            v.push(content);
            page_num += 1;
        }
    }
    Ok(v)
}

fn extract_text_by_page(doc: &Document, page_num: u32) -> PdfResult<String> {
    let mut s = Vec::new();
    {
        let mut output = PlainTextOutput::new(&mut s);
        output_doc_page(doc, &mut output, page_num)?;
    }
    String::from_utf8(s).map_err(|_| PdfError::EncodingError("Invalid UTF-8".to_string()))
}

// Document processing
pub fn print_metadata(doc: &Document) {
    debug!("Version: {}", doc.version);
    if let Some(info) = document_utils::get_info(doc) {
        for (k, v) in info {
            if let Object::String(s, StringFormat::Literal) = v {
                debug!("{}: {}", string_utils::pdf_to_utf8(k).unwrap_or_default(), 
                       string_utils::pdf_to_utf8(s).unwrap_or_default());
            }
        }
    }
    let pages = document_utils::get_pages(doc).ok();
    if let Some(pages) = pages {
        debug!("Page count: {}", get::<i64>(doc, pages, b"Count").unwrap_or(0));
    }
}

pub fn output_doc_encrypted(
    doc: &mut Document,
    output: &mut dyn OutputDev,
    password: &str,
) -> PdfResult<()> {
    doc.decrypt(password)?;
    output_doc(doc, output)
}

pub fn output_doc(doc: &Document, output: &mut dyn OutputDev) -> PdfResult<()> {
    if doc.is_encrypted() {
        error!("Encrypted documents must be decrypted with a password");
    }
    let empty_resources = Dictionary::new();
    let pages = doc.get_pages();
    let mut p = Processor::new();
    for (page_num, object_id) in pages {
        output_doc_inner(page_num, object_id, doc, &mut p, output, &empty_resources)?;
    }
    Ok(())
}

pub fn output_doc_page(doc: &Document, output: &mut dyn OutputDev, page_num: u32) -> PdfResult<()> {
    if doc.is_encrypted() {
        error!("Encrypted documents must be decrypted with a password");
    }
    let empty_resources = Dictionary::new();
    let pages = doc.get_pages();
    let object_id = pages.get(&page_num)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} not found", page_num)))?;
    let mut p = Processor::new();
    output_doc_inner(page_num, *object_id, doc, &mut p, output, &empty_resources)?;
    Ok(())
}

fn output_doc_inner<'a>(
    page_num: u32,
    object_id: ObjectId,
    doc: &'a Document,
    p: &mut Processor<'a>,
    output: &mut dyn OutputDev,
    empty_resources: &'a Dictionary,
) -> PdfResult<()> {
    let page_dict = doc.get_object(object_id)?
        .as_dict()
        .map_err(|_| PdfError::InvalidStructure("Page object must be dictionary".to_string()))?;
    
    let resources = get_inherited(doc, page_dict, b"Resources").unwrap_or(empty_resources);
    let media_box: Vec<f64> = get_inherited(doc, page_dict, b"MediaBox")
        .ok_or_else(|| PdfError::MissingField("MediaBox".to_string()))?;
    
    let media_box = MediaBox {
        llx: media_box[0],
        lly: media_box[1],
        urx: media_box[2],
        ury: media_box[3],
    };
    
    let art_box = get::<Option<Vec<f64>>>(doc, page_dict, b"ArtBox")?
        .map(|x| (x[0], x[1], x[2], x[3]));
    
    output.begin_page(page_num, &media_box, art_box)?;
    p.process_stream(doc, doc.get_page_content(object_id)?, resources, &media_box, output, page_num)?;
    output.end_page()?;
    Ok(())
}

fn get_inherited<'a, T: FromObj<'a>>(doc: &'a Document, dict: &'a Dictionary, key: &[u8]) -> Option<T> {
    let o: Option<T> = get(doc, dict, key).ok();
    if let Some(o) = o {
        Some(o)
    } else {
        let parent = dict.get(b"Parent").ok()?
            .as_reference().ok()?;
        let parent_dict = doc.get_dictionary(parent).ok()?;
        get_inherited(doc, parent_dict, key)
    }
}

// Graphics state
#[derive(Clone)]
struct TextState {
    font: Option<Arc<dyn PdfFont>>,
    font_size: f64,
    character_spacing: f64,
    word_spacing: f64,
    horizontal_scaling: f64,
    leading: f64,
    rise: f64,
    tm: PdfTransform,
}

#[derive(Clone)]
struct GraphicsState {
    ctm: PdfTransform,
    ts: TextState,
    smask: Option<Dictionary>,
    fill_colorspace: ColorSpace,
    fill_color: Vec<f64>,
    stroke_colorspace: ColorSpace,
    stroke_color: Vec<f64>,
    line_width: f64,
}

// Processor for handling PDF content streams
struct Processor<'a> {
    _phantom: PhantomData<&'a ()>,
}

impl<'a> Processor<'a> {
    fn new() -> Self {
        Processor { _phantom: PhantomData }
    }
    
    fn process_stream(
        &mut self,
        doc: &'a Document,
        content: Vec<u8>,
        resources: &'a Dictionary,
        media_box: &MediaBox,
        output: &mut dyn OutputDev,
        page_num: u32,
    ) -> PdfResult<()> {
        let content = Content::decode(&content)
            .map_err(|e| PdfError::InvalidStructure(format!("Failed to decode content: {:?}", e)))?;
        
        let mut font_table = HashMap::new();
        let mut gs = GraphicsState {
            ts: TextState {
                font: None,
                font_size: std::f64::NAN,
                character_spacing: 0.,
                word_spacing: 0.,
                horizontal_scaling: 1.0,
                leading: 0.,
                rise: 0.,
                tm: Transform2D::identity(),
            },
            fill_color: Vec::new(),
            fill_colorspace: ColorSpace::DeviceGray,
            stroke_color: Vec::new(),
            stroke_colorspace: ColorSpace::DeviceGray,
            line_width: 1.,
            ctm: Transform2D::identity(),
            smask: None,
        };
        
        let mut gs_stack = Vec::new();
        let mut mc_stack = Vec::new();
        let mut tlm = Transform2D::identity();
        let mut path = Path::new();
        let flip_ctm = Transform2D::new(1., 0., 0., -1., 0., media_box.ury - media_box.lly);
        
        for operation in &content.operations {
            match operation.operator.as_ref() {
                "BT" => {
                    tlm = Transform2D::identity();
                    gs.ts.tm = tlm;
                }
                "ET" => {
                    tlm = Transform2D::identity();
                    gs.ts.tm = tlm;
                }
                "cm" => {
                    if operation.operands.len() != 6 {
                        return Err(PdfError::InvalidStructure("cm requires 6 operands".to_string()));
                    }
                    let m = Transform2D::new(
                        object_utils::as_num(&operation.operands[0])?,
                        object_utils::as_num(&operation.operands[1])?,
                        object_utils::as_num(&operation.operands[2])?,
                        object_utils::as_num(&operation.operands[3])?,
                        object_utils::as_num(&operation.operands[4])?,
                        object_utils::as_num(&operation.operands[5])?,
                    );
                    gs.ctm = gs.ctm.then(&m);
                }
                "CS" => {
                    let name = operation.operands[0].as_name()
                        .map_err(|_| PdfError::InvalidStructure("CS requires name operand".to_string()))?;
                    gs.stroke_colorspace = make_colorspace(doc, name, resources);
                }
                "cs" => {
                    let name = operation.operands[0].as_name()
                        .map_err(|_| PdfError::InvalidStructure("cs requires name operand".to_string()))?;
                    gs.fill_colorspace = make_colorspace(doc, name, resources);
                }
                "SC" | "SCN" => {
                    gs.stroke_color = match gs.stroke_colorspace {
                        ColorSpace::Pattern => Vec::new(),
                        _ => operation.operands.iter()
                            .map(object_utils::as_num)
                            .collect::<PdfResult<Vec<_>>>()?,
                    };
                }
                "sc" | "scn" => {
                    gs.fill_color = match gs.fill_colorspace {
                        ColorSpace::Pattern => Vec::new(),
                        _ => operation.operands.iter()
                            .map(object_utils::as_num)
                            .collect::<PdfResult<Vec<_>>>()?,
                    };
                }
                "TJ" => {
                    if let Object::Array(array) = &operation.operands[0] {
                        for e in array {
                            match e {
                                Object::String(s, _) => {
                                    show_text(&mut gs, s, &tlm, &flip_ctm, output)?;
                                }
                                Object::Integer(i) => {
                                    let ts = &mut gs.ts;
                                    let w0 = 0.;
                                    let tj = *i as f64;
                                    let ty = 0.;
                                    let tx = ts.horizontal_scaling * ((w0 - tj / 1000.) * ts.font_size);
                                    ts.tm = ts.tm.then(&Transform2D::translation(tx, ty));
                                }
                                Object::Real(f) => {
                                    let ts = &mut gs.ts;
                                    let w0 = 0.;
                                    let tj: f64 = (*f).into();
                                    let ty = 0.;
                                    let tx = ts.horizontal_scaling * ((w0 - tj / 1000.) * ts.font_size);
                                    ts.tm = ts.tm.then(&Transform2D::translation(tx, ty));
                                }
                                _ => {}
                            }
                        }
                    }
                }
                "Tj" => {
                    if let Object::String(s, _) = &operation.operands[0] {
                        show_text(&mut gs, s, &tlm, &flip_ctm, output)?;
                    }
                }
                "Tc" => {
                    gs.ts.character_spacing = object_utils::as_num(&operation.operands[0])?;
                }
                "Tw" => {
                    gs.ts.word_spacing = object_utils::as_num(&operation.operands[0])?;
                }
                "Tz" => {
                    gs.ts.horizontal_scaling = object_utils::as_num(&operation.operands[0])? / 100.;
                }
                "TL" => {
                    gs.ts.leading = object_utils::as_num(&operation.operands[0])?;
                }
                "Tf" => {
                    let fonts: &Dictionary = get(doc, resources, b"Font")?;
                    let name = operation.operands[0].as_name()
                        .map_err(|_| PdfError::InvalidStructure("Tf requires name operand".to_string()))?;
                    let font = font_table.entry(name.to_owned())
                        .or_insert_with(|| make_font(doc, get::<&Dictionary>(doc, fonts, name).unwrap()).unwrap())
                        .clone();
                    gs.ts.font = Some(font);
                    gs.ts.font_size = object_utils::as_num(&operation.operands[1])?;
                }
                "Ts" => {
                    gs.ts.rise = object_utils::as_num(&operation.operands[0])?;
                }
                "Tm" => {
                    if operation.operands.len() != 6 {
                        return Err(PdfError::InvalidStructure("Tm requires 6 operands".to_string()));
                    }
                    tlm = Transform2D::new(
                        object_utils::as_num(&operation.operands[0])?,
                        object_utils::as_num(&operation.operands[1])?,
                        object_utils::as_num(&operation.operands[2])?,
                        object_utils::as_num(&operation.operands[3])?,
                        object_utils::as_num(&operation.operands[4])?,
                        object_utils::as_num(&operation.operands[5])?,
                    );
                    gs.ts.tm = tlm;
                    output.end_line()?;
                }
                "Td" => {
                    if operation.operands.len() != 2 {
                        return Err(PdfError::InvalidStructure("Td requires 2 operands".to_string()));
                    }
                    let tx = object_utils::as_num(&operation.operands[0])?;
                    let ty = object_utils::as_num(&operation.operands[1])?;
                    tlm = tlm.then(&Transform2D::translation(tx, ty));
                    gs.ts.tm = tlm;
                    output.end_line()?;
                }
                "TD" => {
                    if operation.operands.len() != 2 {
                        return Err(PdfError::InvalidStructure("TD requires 2 operands".to_string()));
                    }
                    let tx = object_utils::as_num(&operation.operands[0])?;
                    let ty = object_utils::as_num(&operation.operands[1])?;
                    gs.ts.leading = -ty;
                    tlm = tlm.then(&Transform2D::translation(tx, ty));
                    gs.ts.tm = tlm;
                    output.end_line()?;
                }
                "T*" => {
                    let tx = 0.0;
                    let ty = -gs.ts.leading;
                    tlm = tlm.then(&Transform2D::translation(tx, ty));
                    gs.ts.tm = tlm;
                    output.end_line()?;
                }
                "q" => {
                    gs_stack.push(gs.clone());
                }
                "Q" => {
                    if let Some(s) = gs_stack.pop() {
                        gs = s;
                    } else {
                        warn!("No state to pop");
                    }
                }
                "gs" => {
                    let ext_gstate: &Dictionary = get(doc, resources, b"ExtGState")?;
                    let name = operation.operands[0].as_name()
                        .map_err(|_| PdfError::InvalidStructure("gs requires name operand".to_string()))?;
                    let state: &Dictionary = get(doc, ext_gstate, name)?;
                    apply_state(doc, &mut gs, state)?;
                }
                "m" => {
                    path.ops.push(PathOp::MoveTo(
                        object_utils::as_num(&operation.operands[0])?,
                        object_utils::as_num(&operation.operands[1])?,
                    ));
                }
                "l" => {
                    path.ops.push(PathOp::LineTo(
                        object_utils::as_num(&operation.operands[0])?,
                        object_utils::as_num(&operation.operands[1])?,
                    ));
                }
                "c" => {
                    path.ops.push(PathOp::CurveTo(
                        object_utils::as_num(&operation.operands[0])?,
                        object_utils::as_num(&operation.operands[1])?,
                        object_utils::as_num(&operation.operands[2])?,
                        object_utils::as_num(&operation.operands[3])?,
                        object_utils::as_num(&operation.operands[4])?,
                        object_utils::as_num(&operation.operands[5])?,
                    ));
                }
                "v" => {
                    let (x, y) = path.current_point();
                    path.ops.push(PathOp::CurveTo(
                        x,
                        y,
                        object_utils::as_num(&operation.operands[0])?,
                        object_utils::as_num(&operation.operands[1])?,
                        object_utils::as_num(&operation.operands[2])?,
                        object_utils::as_num(&operation.operands[3])?,
                    ));
                }
                "y" => {
                    path.ops.push(PathOp::CurveTo(
                        object_utils::as_num(&operation.operands[0])?,
                        object_utils::as_num(&operation.operands[1])?,
                        object_utils::as_num(&operation.operands[2])?,
                        object_utils::as_num(&operation.operands[3])?,
                        object_utils::as_num(&operation.operands[2])?,
                        object_utils::as_num(&operation.operands[3])?,
                    ));
                }
                "h" => {
                    path.ops.push(PathOp::Close);
                }
                "re" => {
                    path.ops.push(PathOp::Rect(
                        object_utils::as_num(&operation.operands[0])?,
                        object_utils::as_num(&operation.operands[1])?,
                        object_utils::as_num(&operation.operands[2])?,
                        object_utils::as_num(&operation.operands[3])?,
                    ));
                }
                "S" => {
                    output.stroke(&gs.ctm, &gs.stroke_colorspace, &gs.stroke_color, &path)?;
                    path.ops.clear();
                }
                "F" | "f" => {
                    output.fill(&gs.ctm, &gs.fill_colorspace, &gs.fill_color, &path)?;
                    path.ops.clear();
                }
                "n" => {
                    path.ops.clear();
                }
                "BMC" | "BDC" => {
                    mc_stack.push(operation);
                }
                "EMC" => {
                    mc_stack.pop();
                }
                "Do" => {
                    let xobject: &Dictionary = get(doc, resources, b"XObject")?;
                    let name = operation.operands[0].as_name()
                        .map_err(|_| PdfError::InvalidStructure("Do requires name operand".to_string()))?;
                    let xf: &Stream = get(doc, xobject, name)?;
                    let resources = object_utils::maybe_get_obj(doc, &xf.dict, b"Resources")
                        .and_then(|n| n.as_dict().ok())
                        .unwrap_or(resources);
                    let contents = get_contents(xf);
                    self.process_stream(doc, contents, resources, media_box, output, page_num)?;
                }
                "w" => {
                    gs.line_width = object_utils::as_num(&operation.operands[0])?;
                }
                "G" | "g" | "RG" | "rg" | "K" | "k" => {
                    debug!("Unhandled color operation {:?}", operation);
                }
                "i" | "J" | "j" | "M" | "d" | "ri" => {
                    debug!("Unhandled graphics state operator {:?}", operation);
                }
                "s" | "f*" | "B" | "B*" | "b" => {
                    debug!("Unhandled path op {:?}", operation);
                }
                "W" | "W*" => {
                    debug!("Unhandled clipping operation {:?}", operation);
                }
                _ => {
                    debug!("Unknown operation {:?}", operation);
                }
            }
        }
        Ok(())
    }
}

fn show_text(
    gs: &mut GraphicsState,
    s: &[u8],
    _tlm: &PdfTransform,
    _flip_ctm: &PdfTransform,
    output: &mut dyn OutputDev,
) -> PdfResult<()> {
    let ts = &mut gs.ts;
    let font = ts.font.as_ref()
        .ok_or_else(|| PdfError::InvalidStructure("No font set".to_string()))?;
    
    output.begin_word()?;
    
    let mut iter = s.iter();
    while let Some((c, length)) = font.next_char(&mut iter) {
        let tsm = Transform2D::new(
            ts.horizontal_scaling,
            0.,
            0.,
            1.0,
            0.,
            ts.rise,
        );
        let trm = tsm.then(&ts.tm.then(&gs.ctm));
        
        let w0 = font.get_width(c) / 1000.;
        let mut spacing = ts.character_spacing;
        
        let is_space = c == 32 && length == 1;
        if is_space {
            spacing += ts.word_spacing;
        }
        
        output.output_character(&trm, w0, spacing, ts.font_size, &font.decode_char(c))?;
        
        let tj = 0.;
        let ty = 0.;
        let tx = ts.horizontal_scaling * ((w0 - tj / 1000.) * ts.font_size + spacing);
        ts.tm = ts.tm.then(&Transform2D::translation(tx, ty));
    }
    
    output.end_word()?;
    Ok(())
}

fn apply_state(doc: &Document, gs: &mut GraphicsState, state: &Dictionary) -> PdfResult<()> {
    for (k, v) in state.iter() {
        let k: &[u8] = k.as_ref();
        match k {
            b"SMask" => match object_utils::maybe_deref(doc, v)? {
                Object::Name(name) => {
                    if name == b"None" {
                        gs.smask = None;
                    } else {
                        return Err(PdfError::InvalidStructure("Unexpected smask name".to_string()));
                    }
                }
                Object::Dictionary(dict) => {
                    gs.smask = Some(dict.clone());
                }
                _ => return Err(PdfError::InvalidStructure("Unexpected smask type".to_string())),
            },
            b"Type" => {
                if let Object::Name(name) = v {
                    if name != b"ExtGState" {
                        return Err(PdfError::InvalidStructure("Expected ExtGState type".to_string()));
                    }
                }
            }
            _ => {
                debug!("Unapplied state: {:?} {:?}", k, v);
            }
        }
    }
    Ok(())
}

fn make_colorspace(doc: &Document, name: &[u8], resources: &Dictionary) -> ColorSpace {
    match name {
        b"DeviceGray" => ColorSpace::DeviceGray,
        b"DeviceRGB" => ColorSpace::DeviceRGB,
        b"DeviceCMYK" => ColorSpace::DeviceCMYK,
        b"Pattern" => ColorSpace::Pattern,
        _ => {
            let colorspaces: &Dictionary = get(doc, resources, b"ColorSpace").expect("ColorSpace");
            let cs: &Object = object_utils::maybe_get_obj(doc, colorspaces, name)
                .unwrap_or_else(|| panic!("missing colorspace {:?}", name));
            
            if let Ok(cs) = cs.as_array() {
                let cs_name = string_utils::pdf_to_utf8(cs[0].as_name()
                    .expect("ColorSpace array must start with name")).expect("valid utf8");
                
                match cs_name.as_str() {
                    "Separation" => {
                        let name = string_utils::pdf_to_utf8(cs[1].as_name()
                            .expect("Separation name must be name")).expect("valid utf8");
                        
                        let alternate_space = match object_utils::maybe_deref(doc, &cs[2]).expect("deref") {
                            Object::Name(name) => match &name[..] {
                                b"DeviceGray" => AlternateColorSpace::DeviceGray,
                                b"DeviceRGB" => AlternateColorSpace::DeviceRGB,
                                b"DeviceCMYK" => AlternateColorSpace::DeviceCMYK,
                                _ => panic!("Unknown alternate colorspace"),
                            },
                            Object::Array(cs) => {
                                let cs_name = string_utils::pdf_to_utf8(cs[0].as_name()
                                    .expect("Alternate colorspace must start with name")).expect("valid utf8");
                                
                                match cs_name.as_str() {
                                    "ICCBased" => {
                                        let stream = object_utils::maybe_deref(doc, &cs[1]).expect("deref")
                                            .as_stream()
                                            .expect("ICCBased must have stream");
                                        AlternateColorSpace::ICCBased(get_contents(stream))
                                    }
                                    "CalGray" => {
                                        let dict = cs[1].as_dict()
                                            .expect("CalGray must have dict");
                                        AlternateColorSpace::CalGray(CalGray {
                                            white_point: get(doc, dict, b"WhitePoint").expect("WhitePoint"),
                                            black_point: get(doc, dict, b"BlackPoint").ok(),
                                            gamma: get(doc, dict, b"Gamma").ok(),
                                        })
                                    }
                                    "CalRGB" => {
                                        let dict = cs[1].as_dict()
                                            .expect("CalRGB must have dict");
                                        AlternateColorSpace::CalRGB(CalRGB {
                                            white_point: get(doc, dict, b"WhitePoint").expect("WhitePoint"),
                                            black_point: get(doc, dict, b"BlackPoint").ok(),
                                            gamma: get(doc, dict, b"Gamma").ok(),
                                            matrix: get(doc, dict, b"Matrix").ok(),
                                        })
                                    }
                                    "Lab" => {
                                        let dict = cs[1].as_dict()
                                            .expect("Lab must have dict");
                                        AlternateColorSpace::Lab(Lab {
                                            white_point: get(doc, dict, b"WhitePoint").expect("WhitePoint"),
                                            black_point: get(doc, dict, b"BlackPoint").ok(),
                                            range: get(doc, dict, b"Range").ok(),
                                        })
                                    }
                                    _ => panic!("Unknown alternate colorspace"),
                                }
                            }
                            _ => panic!("Alternate space must be name or array"),
                        };
                        
                        let tint_transform = Box::new(Function::new(doc, object_utils::maybe_deref(doc, &cs[3]).expect("deref")).expect("Function"));
                        
                        ColorSpace::Separation(Separation {
                            name,
                            alternate_space,
                            tint_transform,
                        })
                    }
                    "ICCBased" => {
                        let stream = object_utils::maybe_deref(doc, &cs[1]).expect("deref")
                            .as_stream()
                            .expect("ICCBased must have stream");
                        ColorSpace::ICCBased(get_contents(stream))
                    }
                    "CalGray" => {
                        let dict = cs[1].as_dict()
                            .expect("CalGray must have dict");
                        ColorSpace::CalGray(CalGray {
                            white_point: get(doc, dict, b"WhitePoint").expect("WhitePoint"),
                            black_point: get(doc, dict, b"BlackPoint").ok(),
                            gamma: get(doc, dict, b"Gamma").ok(),
                        })
                    }
                    "CalRGB" => {
                        let dict = cs[1].as_dict()
                            .expect("CalRGB must have dict");
                        ColorSpace::CalRGB(CalRGB {
                            white_point: get(doc, dict, b"WhitePoint").expect("WhitePoint"),
                            black_point: get(doc, dict, b"BlackPoint").ok(),
                            gamma: get(doc, dict, b"Gamma").ok(),
                            matrix: get(doc, dict, b"Matrix").ok(),
                        })
                    }
                    "Lab" => {
                        let dict = cs[1].as_dict()
                            .expect("Lab must have dict");
                        ColorSpace::Lab(Lab {
                            white_point: get(doc, dict, b"WhitePoint").expect("WhitePoint"),
                            black_point: get(doc, dict, b"BlackPoint").ok(),
                            range: get(doc, dict, b"Range").ok(),
                        })
                    }
                    "Pattern" => ColorSpace::Pattern,
                    "DeviceGray" => ColorSpace::DeviceGray,
                    "DeviceRGB" => ColorSpace::DeviceRGB,
                    "DeviceCMYK" => ColorSpace::DeviceCMYK,
                    "DeviceN" => ColorSpace::DeviceN,
                    _ => panic!("Unknown colorspace: {}", cs_name),
                }
            } else if let Ok(cs) = cs.as_name() {
                match string_utils::pdf_to_utf8(cs).expect("valid utf8").as_str() {
                    "DeviceRGB" => ColorSpace::DeviceRGB,
                    "DeviceGray" => ColorSpace::DeviceGray,
                    _ => panic!("Unknown colorspace name"),
                }
            } else {
                panic!("ColorSpace must be name or array")
            }
        }
    }
}

// Backward compatibility type alias
pub type OutputError = PdfError;
