use crate::{Id, Image};
use emath::Vec2;
use epaint::Galley;
use std::rc::Rc;

/// A sized [`crate::AtomKind`].
#[derive(Clone, Default, Debug)]
pub enum SizedAtomKind<'a> {
    #[default]
    Empty,
    Text(Rc<Galley>),
    Image(Image<'a>, Vec2),
    Custom(Id),
}

impl SizedAtomKind<'_> {
    /// Get the calculated size.
    pub fn size(&self) -> Vec2 {
        match self {
            SizedAtomKind::Text(galley) => galley.size(),
            SizedAtomKind::Image(_, size) => *size,
            SizedAtomKind::Empty | SizedAtomKind::Custom(_) => Vec2::ZERO,
        }
    }
}
