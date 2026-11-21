//! Objects of the case language that a line makes in a numbered slot (`tex 3 ...`, `rtv 0 ...`)
//! and other objects hold on to (a view its resource, the device's bindings their views).
//!
//! A slot is where the next line finds an object, and a later line may put another one there;
//! what holds an object keeps that object, as COM's references do: the executor releases its
//! own pointer in the slot, the view or the binding still has the old object (the review of
//! 8480456, 2.1: a view kept the slot number, and clearing it after the slot was reused wrote
//! the new texture). [`Slot`] and [`Id`] are different types, so code cannot take one for the
//! other, and an [`Id`] is only made by [`Objects::put`], so it always names an object.
use std::marker::PhantomData;

use super::{Fail, Res};

/// how many slots each kind has (the executors' SLOTS)
pub const SLOTS: usize = 16;

/// a slot of objects of kind `T`: 0 .. SLOTS
pub struct Slot<T>(usize, PhantomData<fn() -> T>);

/// an object of kind `T` once made, whatever its slot holds later
pub struct Id<T>(usize, PhantomData<fn() -> T>);

impl<T> Clone for Slot<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Slot<T> {}
impl<T> PartialEq for Slot<T> {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}
impl<T> Eq for Slot<T> {}
impl<T> std::fmt::Debug for Slot<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Slot({})", self.0)
    }
}
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}
impl<T> PartialEq for Id<T> {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}
impl<T> Eq for Id<T> {}
impl<T> std::fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Id({})", self.0)
    }
}

impl<T> Slot<T> {
    /// slot `n` (past the last: a line the executor refuses, Fail::Bad)
    pub fn new(n: u64) -> Res<Self> {
        let n = usize::try_from(n).ok().filter(|&n| n < SLOTS).ok_or(Fail::Bad)?;
        Ok(Slot(n, PhantomData))
    }
    /// the slot's number
    pub fn number(self) -> usize {
        self.0
    }
}

/// the slots of one kind and every object put in them during a case (an object a slot let go
/// of may still be held: they are dropped with the case)
pub struct Objects<T> {
    slots: [Option<Id<T>>; SLOTS],
    made: Vec<T>,
}

impl<T> Default for Objects<T> {
    fn default() -> Self {
        Objects { slots: [None; SLOTS], made: Vec::new() }
    }
}

impl<T> Objects<T> {
    /// the object in slot `s`
    pub fn id(&self, s: Slot<T>) -> Option<Id<T>> {
        self.slots.get(s.0).copied().flatten()
    }
    /// the object in slot `s`
    pub fn at(&self, s: Slot<T>) -> Option<&T> {
        self.id(s).and_then(|i| self.made.get(i.0))
    }
    /// the object in slot `s`, to change
    pub fn at_mut(&mut self, s: Slot<T>) -> Option<&mut T> {
        self.id(s).and_then(|i| self.made.get_mut(i.0))
    }
    /// an object made (every Id is one of these objects': Fail::Bad would be a model error)
    pub fn get(&self, i: Id<T>) -> Res<&T> {
        self.made.get(i.0).ok_or(Fail::Bad)
    }
    /// an object made, to change
    pub fn get_mut(&mut self, i: Id<T>) -> Res<&mut T> {
        self.made.get_mut(i.0).ok_or(Fail::Bad)
    }
    /// a new object into slot `s` (the old one stays for what holds it)
    pub fn put(&mut self, s: Slot<T>, v: T) -> Id<T> {
        let id = Id(self.made.len(), PhantomData);
        self.made.push(v);
        if let Some(x) = self.slots.get_mut(s.0) {
            *x = Some(id);
        }
        id
    }
    /// slot `s` lets go of its object
    pub fn empty(&mut self, s: Slot<T>) {
        if let Some(x) = self.slots.get_mut(s.0) {
            *x = None;
        }
    }
}
