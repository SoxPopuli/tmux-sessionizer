use crate::{
    config::SearchPath,
    error::{CacheError, Error},
};
use std::io::{Read, Write};

fn write<W: Write>(writer: &mut W, elem_type: &'static str, data: &[u8]) -> Result<(), Error> {
    writer
        .write_all(data)
        .map_err(|e| Error::Cache(CacheError::Write(elem_type, e)))
}

fn read_n<R: Read, const N: usize>(
    reader: &mut R,
    elem_type: &'static str,
) -> Result<[u8; N], Error> {
    let mut buf = [0; N];
    match reader.read_exact(&mut buf) {
        Ok(_) => Ok(buf),
        Err(e) => Err(Error::Cache(CacheError::Read(elem_type, e))),
    }
}

pub trait WriteBinary {
    fn write_binary<W: Write>(&self, writer: &mut W) -> Result<(), Error>;
}

pub trait ReadBinary: Sized {
    fn read_binary<R: Read>(reader: &mut R) -> Result<Self, Error>;
}

impl WriteBinary for String {
    fn write_binary<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        let len = self.len();

        write(writer, "String length", &len.to_ne_bytes())?;
        write(writer, "String data", self.as_bytes())?;

        Ok(())
    }
}

impl ReadBinary for String {
    fn read_binary<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let length = {
            let len = read_n(reader, "String length")?;
            usize::from_ne_bytes(len)
        };
        let mut buf = vec![0; length];
        reader
            .read_exact(&mut buf)
            .map_err(|e| Error::Cache(CacheError::Read("String data", e)))?;
        let data = String::from_utf8(buf).map_err(|e| {
            Error::Cache(CacheError::Read(
                "String data",
                std::io::Error::other(format!("Invalid utf-8 string: {e}")),
            ))
        })?;

        Ok(data)
    }
}

impl<T> WriteBinary for Option<T>
where
    T: WriteBinary,
{
    fn write_binary<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        match self {
            Some(elem) => {
                write(writer, "Option byte", &[1])?;
                elem.write_binary(writer)
            }
            None => write(writer, "Option byte", &[0]),
        }
    }
}

impl<T> ReadBinary for Option<T>
where
    T: ReadBinary,
{
    fn read_binary<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let option_byte: [u8; 1] = read_n(reader, "Option byte")?;
        if option_byte[0] == 1 {
            T::read_binary(reader).map(Some)
        } else {
            Ok(None)
        }
    }
}

impl<T> WriteBinary for Vec<T>
where
    T: WriteBinary,
{
    fn write_binary<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        let length = self.len();
        write(writer, "Vec length", &length.to_ne_bytes())?;
        for elem in self {
            elem.write_binary(writer)?;
        }

        Ok(())
    }
}

impl<T> ReadBinary for Vec<T>
where
    T: ReadBinary,
{
    fn read_binary<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let length = {
            let len = read_n(reader, "Vec length")?;
            usize::from_ne_bytes(len)
        };

        let mut v = Vec::with_capacity(length);

        for _ in 0..length {
            v.push(T::read_binary(reader)?);
        }

        Ok(v)
    }
}

impl WriteBinary for u8 {
    fn write_binary<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        write(writer, "u8", &[*self])
    }
}

impl ReadBinary for u8 {
    fn read_binary<R: Read>(reader: &mut R) -> Result<Self, Error> {
        read_n(reader, "u8").map(|x: [u8; 1]| x[0])
    }
}

impl WriteBinary for bool {
    fn write_binary<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        let byte = match self {
            false => 0,
            true => 1,
        };

        write(writer, "bool", &[byte])
    }
}

impl ReadBinary for bool {
    fn read_binary<R: Read>(reader: &mut R) -> Result<Self, Error> {
        read_n(reader, "bool").map(|x: [u8; 1]| !matches!(x[0], 0))
    }
}

impl WriteBinary for crate::config::Settings {
    fn write_binary<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        write(writer, "u8", &[self.default_depth])?;
        self.picker.write_binary(writer)?;

        Ok(())
    }
}

impl ReadBinary for crate::config::Settings {
    fn read_binary<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let default_depth: [u8; 1] = read_n(reader, "u8")?;
        let picker = Option::<String>::read_binary(reader)?;

        Ok(crate::config::Settings {
            default_depth: default_depth[0],
            picker,
        })
    }
}

mod search_path {
    #![allow(non_upper_case_globals)]

    pub const Simple: u8 = 0;
    pub const Complex: u8 = 1;
}

impl WriteBinary for SearchPath {
    fn write_binary<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        match self {
            SearchPath::Simple(data) => {
                write(
                    writer,
                    "SearchPath::Simple type byte",
                    &[search_path::Simple],
                )?;
                data.write_binary(writer)
            }
            SearchPath::Complex {
                path,
                depth,
                show_hidden,
            } => {
                write(
                    writer,
                    "SearchPath::Complex type byte",
                    &[search_path::Complex],
                )?;
                path.write_binary(writer)?;
                depth.write_binary(writer)?;
                show_hidden.write_binary(writer)
            }
        }
    }
}

impl ReadBinary for SearchPath {
    fn read_binary<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let byte: [u8; 1] = read_n(reader, "SearchPath type byte")?;
        match byte[0] {
            search_path::Simple => Ok(SearchPath::Simple(String::read_binary(reader)?)),
            search_path::Complex => Ok(Self::Complex {
                path: String::read_binary(reader)?,
                depth: Option::<u8>::read_binary(reader)?,
                show_hidden: Option::<bool>::read_binary(reader)?,
            }),

            x => Err(Error::Cache(CacheError::Read(
                "SearchPath type byte",
                std::io::Error::other(format!("Invalid SearchPath type byte: {x}")),
            ))),
        }
    }
}

impl WriteBinary for crate::config::Config {
    fn write_binary<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        self.settings.write_binary(writer)?;
        self.paths.write_binary(writer)?;

        Ok(())
    }
}

impl ReadBinary for crate::config::Config {
    fn read_binary<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let settings = crate::config::Settings::read_binary(reader)?;
        let paths = Vec::<SearchPath>::read_binary(reader)?;

        Ok(crate::config::Config { paths, settings })
    }
}
