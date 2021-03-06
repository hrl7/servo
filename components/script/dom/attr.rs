/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use dom::bindings::cell::{DOMRefCell, Ref};
use dom::bindings::codegen::Bindings::AttrBinding;
use dom::bindings::codegen::Bindings::AttrBinding::AttrMethods;
use dom::bindings::codegen::InheritTypes::NodeCast;
use dom::bindings::global;
use dom::bindings::js::{JS, JSRef, Temporary};
use dom::bindings::utils::{Reflectable, Reflector, reflect_dom_object};
use dom::element::{Element, AttributeHandlers};
use dom::node::Node;
use dom::window::Window;
use dom::virtualmethods::vtable_for;

use devtools_traits::AttrInfo;
use servo_util::str::{DOMString, split_html_space_chars};
use std::mem;
use string_cache::{Atom, Namespace};

pub enum AttrSettingType {
    FirstSetAttr,
    ReplacedAttr,
}

#[deriving(PartialEq, Clone)]
#[jstraceable]
pub enum AttrValue {
    StringAttrValue(DOMString),
    TokenListAttrValue(DOMString, Vec<Atom>),
    UIntAttrValue(DOMString, u32),
    AtomAttrValue(Atom),
}

impl AttrValue {
    pub fn from_tokenlist(tokens: DOMString) -> AttrValue {
        let atoms = split_html_space_chars(tokens.as_slice())
            .map(|token| Atom::from_slice(token)).collect();
        TokenListAttrValue(tokens, atoms)
    }

    pub fn from_u32(string: DOMString, default: u32) -> AttrValue {
        let result: u32 = from_str(string.as_slice()).unwrap_or(default);
        UIntAttrValue(string, result)
    }

    pub fn from_atomic(string: DOMString) -> AttrValue {
        let value = Atom::from_slice(string.as_slice());
        AtomAttrValue(value)
    }

    pub fn tokens<'a>(&'a self) -> Option<&'a [Atom]> {
        match *self {
            TokenListAttrValue(_, ref tokens) => Some(tokens.as_slice()),
            _ => None
        }
    }
}

impl Str for AttrValue {
    fn as_slice<'a>(&'a self) -> &'a str {
        match *self {
            StringAttrValue(ref value) |
            TokenListAttrValue(ref value, _) |
            UIntAttrValue(ref value, _) => value.as_slice(),
            AtomAttrValue(ref value) => value.as_slice(),
        }
    }
}

#[dom_struct]
pub struct Attr {
    reflector_: Reflector,
    local_name: Atom,
    value: DOMRefCell<AttrValue>,
    name: Atom,
    namespace: Namespace,
    prefix: Option<DOMString>,

    /// the element that owns this attribute.
    owner: JS<Element>,
}

impl Reflectable for Attr {
    fn reflector<'a>(&'a self) -> &'a Reflector {
        &self.reflector_
    }
}

impl Attr {
    fn new_inherited(local_name: Atom, value: AttrValue,
                     name: Atom, namespace: Namespace,
                     prefix: Option<DOMString>, owner: JSRef<Element>) -> Attr {
        Attr {
            reflector_: Reflector::new(),
            local_name: local_name,
            value: DOMRefCell::new(value),
            name: name,
            namespace: namespace,
            prefix: prefix,
            owner: JS::from_rooted(owner),
        }
    }

    pub fn new(window: JSRef<Window>, local_name: Atom, value: AttrValue,
               name: Atom, namespace: Namespace,
               prefix: Option<DOMString>, owner: JSRef<Element>) -> Temporary<Attr> {
        reflect_dom_object(box Attr::new_inherited(local_name, value, name, namespace, prefix, owner),
                           &global::Window(window), AttrBinding::Wrap)
    }

    #[inline]
    pub fn name<'a>(&'a self) -> &'a Atom {
        &self.name
    }

    #[inline]
    pub fn namespace<'a>(&'a self) -> &'a Namespace {
        &self.namespace
    }

    #[inline]
    pub fn prefix<'a>(&'a self) -> &'a Option<DOMString> {
        &self.prefix
    }
}

impl<'a> AttrMethods for JSRef<'a, Attr> {
    fn LocalName(self) -> DOMString {
        self.local_name().as_slice().to_string()
    }

    fn Value(self) -> DOMString {
        self.value().as_slice().to_string()
    }

    fn SetValue(self, value: DOMString) {
        let owner = self.owner.root();
        let value = owner.parse_attribute(&self.namespace, self.local_name(), value);
        self.set_value(ReplacedAttr, value);
    }

    fn TextContent(self) -> DOMString {
        self.Value()
    }

    fn SetTextContent(self, value: DOMString) {
        self.SetValue(value)
    }

    fn Name(self) -> DOMString {
        self.name.as_slice().to_string()
    }

    fn GetNamespaceURI(self) -> Option<DOMString> {
        let Namespace(ref atom) = self.namespace;
        match atom.as_slice() {
            "" => None,
            url => Some(url.to_string()),
        }
    }

    fn GetPrefix(self) -> Option<DOMString> {
        self.prefix.clone()
    }

    fn GetOwnerElement(self) -> Option<Temporary<Element>> {
        Some(Temporary::new(self.owner))
    }

    fn Specified(self) -> bool {
        true // Always returns true
    }
}

pub trait AttrHelpers<'a> {
    fn set_value(self, set_type: AttrSettingType, value: AttrValue);
    fn value(self) -> Ref<'a, AttrValue>;
    fn local_name(self) -> &'a Atom;
    fn summarize(self) -> AttrInfo;
}

impl<'a> AttrHelpers<'a> for JSRef<'a, Attr> {
    fn set_value(self, set_type: AttrSettingType, value: AttrValue) {
        let owner = self.owner.root();
        let node: JSRef<Node> = NodeCast::from_ref(*owner);
        let namespace_is_null = self.namespace == ns!("");

        match set_type {
            ReplacedAttr if namespace_is_null => vtable_for(&node).before_remove_attr(self),
            _ => ()
        }

        *self.value.borrow_mut() = value;

        if namespace_is_null {
            vtable_for(&node).after_set_attr(self)
        }
    }

    fn value(self) -> Ref<'a, AttrValue> {
        self.extended_deref().value.borrow()
    }

    fn local_name(self) -> &'a Atom {
        &self.extended_deref().local_name
    }

    fn summarize(self) -> AttrInfo {
        let Namespace(ref ns) = self.namespace;
        AttrInfo {
            namespace: ns.as_slice().to_string(),
            name: self.Name(),
            value: self.Value(),
        }
    }
}

pub trait AttrHelpersForLayout {
    unsafe fn value_ref_forever(&self) -> &'static str;
    unsafe fn value_atom_forever(&self) -> Option<Atom>;
    unsafe fn value_tokens_forever(&self) -> Option<&'static [Atom]>;
    unsafe fn local_name_atom_forever(&self) -> Atom;
}

impl AttrHelpersForLayout for Attr {
    #[inline]
    unsafe fn value_ref_forever(&self) -> &'static str {
        // This transmute is used to cheat the lifetime restriction.
        let value = mem::transmute::<&AttrValue, &AttrValue>(self.value.borrow_for_layout());
        value.as_slice()
    }

    #[inline]
    unsafe fn value_atom_forever(&self) -> Option<Atom> {
        let value = self.value.borrow_for_layout();
        match *value {
            AtomAttrValue(ref val) => Some(val.clone()),
            _ => None,
        }
    }

    #[inline]
    unsafe fn value_tokens_forever(&self) -> Option<&'static [Atom]> {
        // This transmute is used to cheat the lifetime restriction.
        let value = mem::transmute::<&AttrValue, &AttrValue>(self.value.borrow_for_layout());
        match *value {
            TokenListAttrValue(_, ref tokens) => Some(tokens.as_slice()),
            _ => None,
        }
    }

    #[inline]
    unsafe fn local_name_atom_forever(&self) -> Atom {
        self.local_name.clone()
    }
}
