/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use dom_struct::dom_struct;
use html5ever::{local_name, namespace_url, ns, LocalName, Prefix};
use js::rust::HandleObject;
use style::attr::{parse_unsigned_integer, AttrValue, LengthOrPercentageOrAuto};
use style::color::parsing::RgbaLegacy;

use crate::dom::attr::Attr;
use crate::dom::bindings::codegen::Bindings::HTMLCollectionBinding::HTMLCollectionMethods;
use crate::dom::bindings::codegen::Bindings::HTMLTableElementBinding::HTMLTableElementMethods;
use crate::dom::bindings::codegen::Bindings::NodeBinding::NodeMethods;
use crate::dom::bindings::error::{Error, ErrorResult, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{Dom, DomRoot, LayoutDom, MutNullableDom};
use crate::dom::bindings::str::DOMString;
use crate::dom::document::Document;
use crate::dom::element::{AttributeMutation, Element, LayoutElementHelpers};
use crate::dom::htmlcollection::{CollectionFilter, HTMLCollection};
use crate::dom::htmlelement::HTMLElement;
use crate::dom::htmltablecaptionelement::HTMLTableCaptionElement;
use crate::dom::htmltablecolelement::HTMLTableColElement;
use crate::dom::htmltablerowelement::HTMLTableRowElement;
use crate::dom::htmltablesectionelement::HTMLTableSectionElement;
use crate::dom::node::{document_from_node, window_from_node, Node};
use crate::dom::virtualmethods::VirtualMethods;

#[dom_struct]
pub struct HTMLTableElement {
    htmlelement: HTMLElement,
    border: Cell<Option<u32>>,
    cellpadding: Cell<Option<u32>>,
    cellspacing: Cell<Option<u32>>,
    tbodies: MutNullableDom<HTMLCollection>,
}

#[allow(crown::unrooted_must_root)]
#[derive(JSTraceable, MallocSizeOf)]
struct TableRowFilter {
    sections: Vec<Dom<Node>>,
}

impl CollectionFilter for TableRowFilter {
    fn filter(&self, elem: &Element, root: &Node) -> bool {
        elem.is::<HTMLTableRowElement>() &&
            (root.is_parent_of(elem.upcast()) ||
                self.sections
                    .iter()
                    .any(|section| section.is_parent_of(elem.upcast())))
    }
}

impl HTMLTableElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> HTMLTableElement {
        HTMLTableElement {
            htmlelement: HTMLElement::new_inherited(local_name, prefix, document),
            border: Cell::new(None),
            cellpadding: Cell::new(None),
            cellspacing: Cell::new(None),
            tbodies: Default::default(),
        }
    }

    #[allow(crown::unrooted_must_root)]
    pub fn new(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<HTMLTableElement> {
        let n = Node::reflect_node_with_proto(
            Box::new(HTMLTableElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        );

        n.upcast::<Node>().set_weird_parser_insertion_mode();
        n
    }

    pub fn get_border(&self) -> Option<u32> {
        self.border.get()
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-thead
    // https://html.spec.whatwg.org/multipage/#dom-table-tfoot
    fn get_first_section_of_type(
        &self,
        atom: &LocalName,
    ) -> Option<DomRoot<HTMLTableSectionElement>> {
        self.upcast::<Node>()
            .child_elements()
            .find(|n| n.is::<HTMLTableSectionElement>() && n.local_name() == atom)
            .and_then(|n| n.downcast().map(DomRoot::from_ref))
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-thead
    // https://html.spec.whatwg.org/multipage/#dom-table-tfoot
    fn set_first_section_of_type<P>(
        &self,
        atom: &LocalName,
        section: Option<&HTMLTableSectionElement>,
        reference_predicate: P,
    ) -> ErrorResult
    where
        P: FnMut(&DomRoot<Element>) -> bool,
    {
        if let Some(e) = section {
            if e.upcast::<Element>().local_name() != atom {
                return Err(Error::HierarchyRequest);
            }
        }

        self.delete_first_section_of_type(atom);

        let node = self.upcast::<Node>();

        if let Some(section) = section {
            let reference_element = node.child_elements().find(reference_predicate);
            let reference_node = reference_element.as_ref().map(|e| e.upcast());

            node.InsertBefore(section.upcast(), reference_node)?;
        }

        Ok(())
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-createthead
    // https://html.spec.whatwg.org/multipage/#dom-table-createtfoot
    fn create_section_of_type(&self, atom: &LocalName) -> DomRoot<HTMLTableSectionElement> {
        if let Some(section) = self.get_first_section_of_type(atom) {
            return section;
        }

        let section =
            HTMLTableSectionElement::new(atom.clone(), None, &document_from_node(self), None);
        match *atom {
            local_name!("thead") => self.SetTHead(Some(&section)),
            local_name!("tfoot") => self.SetTFoot(Some(&section)),
            _ => unreachable!("unexpected section type"),
        }
        .expect("unexpected section type");

        section
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-deletethead
    // https://html.spec.whatwg.org/multipage/#dom-table-deletetfoot
    fn delete_first_section_of_type(&self, atom: &LocalName) {
        if let Some(thead) = self.get_first_section_of_type(atom) {
            thead.upcast::<Node>().remove_self();
        }
    }

    fn get_rows(&self) -> TableRowFilter {
        TableRowFilter {
            sections: self
                .upcast::<Node>()
                .children()
                .filter_map(|ref node| {
                    node.downcast::<HTMLTableSectionElement>()
                        .map(|_| Dom::from_ref(&**node))
                })
                .collect(),
        }
    }
}

impl HTMLTableElementMethods for HTMLTableElement {
    // https://html.spec.whatwg.org/multipage/#dom-table-rows
    fn Rows(&self) -> DomRoot<HTMLCollection> {
        let filter = self.get_rows();
        HTMLCollection::new(&window_from_node(self), self.upcast(), Box::new(filter))
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-caption
    fn GetCaption(&self) -> Option<DomRoot<HTMLTableCaptionElement>> {
        self.upcast::<Node>()
            .children()
            .filter_map(DomRoot::downcast)
            .next()
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-caption
    fn SetCaption(&self, new_caption: Option<&HTMLTableCaptionElement>) -> Fallible<()> {
        if let Some(ref caption) = self.GetCaption() {
            caption.upcast::<Node>().remove_self();
        }

        if let Some(caption) = new_caption {
            let node = self.upcast::<Node>();
            node.InsertBefore(caption.upcast(), node.GetFirstChild().as_deref())?;
        }

        Ok(())
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-createcaption
    fn CreateCaption(&self) -> DomRoot<HTMLTableCaptionElement> {
        match self.GetCaption() {
            Some(caption) => caption,
            None => {
                let caption = HTMLTableCaptionElement::new(
                    local_name!("caption"),
                    None,
                    &document_from_node(self),
                    None,
                );
                self.SetCaption(Some(&caption))
                    .expect("Generated caption is invalid");
                caption
            },
        }
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-deletecaption
    fn DeleteCaption(&self) {
        if let Some(caption) = self.GetCaption() {
            caption.upcast::<Node>().remove_self();
        }
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-thead
    fn GetTHead(&self) -> Option<DomRoot<HTMLTableSectionElement>> {
        self.get_first_section_of_type(&local_name!("thead"))
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-thead
    fn SetTHead(&self, thead: Option<&HTMLTableSectionElement>) -> ErrorResult {
        self.set_first_section_of_type(&local_name!("thead"), thead, |n| {
            !n.is::<HTMLTableCaptionElement>() && !n.is::<HTMLTableColElement>()
        })
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-createthead
    fn CreateTHead(&self) -> DomRoot<HTMLTableSectionElement> {
        self.create_section_of_type(&local_name!("thead"))
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-deletethead
    fn DeleteTHead(&self) {
        self.delete_first_section_of_type(&local_name!("thead"))
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-tfoot
    fn GetTFoot(&self) -> Option<DomRoot<HTMLTableSectionElement>> {
        self.get_first_section_of_type(&local_name!("tfoot"))
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-tfoot
    fn SetTFoot(&self, tfoot: Option<&HTMLTableSectionElement>) -> ErrorResult {
        self.set_first_section_of_type(&local_name!("tfoot"), tfoot, |n| {
            if n.is::<HTMLTableCaptionElement>() || n.is::<HTMLTableColElement>() {
                return false;
            }

            if n.is::<HTMLTableSectionElement>() {
                let name = n.local_name();
                if name == &local_name!("thead") || name == &local_name!("tbody") {
                    return false;
                }
            }

            true
        })
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-createtfoot
    fn CreateTFoot(&self) -> DomRoot<HTMLTableSectionElement> {
        self.create_section_of_type(&local_name!("tfoot"))
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-deletetfoot
    fn DeleteTFoot(&self) {
        self.delete_first_section_of_type(&local_name!("tfoot"))
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-tbodies
    fn TBodies(&self) -> DomRoot<HTMLCollection> {
        #[derive(JSTraceable)]
        struct TBodiesFilter;
        impl CollectionFilter for TBodiesFilter {
            fn filter(&self, elem: &Element, root: &Node) -> bool {
                elem.is::<HTMLTableSectionElement>() &&
                    elem.local_name() == &local_name!("tbody") &&
                    elem.upcast::<Node>().GetParentNode().as_deref() == Some(root)
            }
        }

        self.tbodies.or_init(|| {
            let window = window_from_node(self);
            let filter = Box::new(TBodiesFilter);
            HTMLCollection::create(&window, self.upcast(), filter)
        })
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-createtbody
    fn CreateTBody(&self) -> DomRoot<HTMLTableSectionElement> {
        let tbody = HTMLTableSectionElement::new(
            local_name!("tbody"),
            None,
            &document_from_node(self),
            None,
        );
        let node = self.upcast::<Node>();
        let last_tbody = node
            .rev_children()
            .filter_map(DomRoot::downcast::<Element>)
            .find(|n| n.is::<HTMLTableSectionElement>() && n.local_name() == &local_name!("tbody"));
        let reference_element = last_tbody.and_then(|t| t.upcast::<Node>().GetNextSibling());

        node.InsertBefore(tbody.upcast(), reference_element.as_deref())
            .expect("Insertion failed");
        tbody
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-insertrow
    fn InsertRow(&self, index: i32) -> Fallible<DomRoot<HTMLTableRowElement>> {
        let rows = self.Rows();
        let number_of_row_elements = rows.Length();

        if index < -1 || index > number_of_row_elements as i32 {
            return Err(Error::IndexSize);
        }

        let new_row =
            HTMLTableRowElement::new(local_name!("tr"), None, &document_from_node(self), None);
        let node = self.upcast::<Node>();

        if number_of_row_elements == 0 {
            // append new row to last or new tbody in table
            if let Some(last_tbody) = node
                .rev_children()
                .filter_map(DomRoot::downcast::<Element>)
                .find(|n| {
                    n.is::<HTMLTableSectionElement>() && n.local_name() == &local_name!("tbody")
                })
            {
                last_tbody
                    .upcast::<Node>()
                    .AppendChild(new_row.upcast::<Node>())
                    .expect("InsertRow failed to append first row.");
            } else {
                let tbody = self.CreateTBody();
                node.AppendChild(tbody.upcast())
                    .expect("InsertRow failed to append new tbody.");

                tbody
                    .upcast::<Node>()
                    .AppendChild(new_row.upcast::<Node>())
                    .expect("InsertRow failed to append first row.");
            }
        } else if index == number_of_row_elements as i32 || index == -1 {
            // append new row to parent of last row in table
            let last_row = rows
                .Item(number_of_row_elements - 1)
                .expect("InsertRow failed to find last row in table.");

            let last_row_parent = last_row
                .upcast::<Node>()
                .GetParentNode()
                .expect("InsertRow failed to find parent of last row in table.");

            last_row_parent
                .upcast::<Node>()
                .AppendChild(new_row.upcast::<Node>())
                .expect("InsertRow failed to append last row.");
        } else {
            // insert new row before the index-th row in rows using the same parent
            let ith_row = rows
                .Item(index as u32)
                .expect("InsertRow failed to find a row in table.");

            let ith_row_parent = ith_row
                .upcast::<Node>()
                .GetParentNode()
                .expect("InsertRow failed to find parent of a row in table.");

            ith_row_parent
                .upcast::<Node>()
                .InsertBefore(new_row.upcast::<Node>(), Some(ith_row.upcast::<Node>()))
                .expect("InsertRow failed to append row");
        }

        Ok(new_row)
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-deleterow
    fn DeleteRow(&self, mut index: i32) -> Fallible<()> {
        let rows = self.Rows();
        // Step 1.
        if index == -1 {
            index = rows.Length() as i32 - 1;
        }
        // Step 2.
        if index < 0 || index as u32 >= rows.Length() {
            return Err(Error::IndexSize);
        }
        // Step 3.
        DomRoot::upcast::<Node>(rows.Item(index as u32).unwrap()).remove_self();
        Ok(())
    }

    // https://html.spec.whatwg.org/multipage/#dom-table-bgcolor
    make_getter!(BgColor, "bgcolor");

    // https://html.spec.whatwg.org/multipage/#dom-table-bgcolor
    make_legacy_color_setter!(SetBgColor, "bgcolor");

    // https://html.spec.whatwg.org/multipage/#dom-table-width
    make_getter!(Width, "width");

    // https://html.spec.whatwg.org/multipage/#dom-table-width
    make_nonzero_dimension_setter!(SetWidth, "width");
}

pub trait HTMLTableElementLayoutHelpers {
    fn get_background_color(self) -> Option<RgbaLegacy>;
    fn get_border(self) -> Option<u32>;
    fn get_cellpadding(self) -> Option<u32>;
    fn get_cellspacing(self) -> Option<u32>;
    fn get_width(self) -> LengthOrPercentageOrAuto;
}

impl HTMLTableElementLayoutHelpers for LayoutDom<'_, HTMLTableElement> {
    fn get_background_color(self) -> Option<RgbaLegacy> {
        self.upcast::<Element>()
            .get_attr_for_layout(&ns!(), &local_name!("bgcolor"))
            .and_then(AttrValue::as_color)
            .cloned()
    }

    fn get_border(self) -> Option<u32> {
        (self.unsafe_get()).border.get()
    }

    fn get_cellpadding(self) -> Option<u32> {
        (self.unsafe_get()).cellpadding.get()
    }

    fn get_cellspacing(self) -> Option<u32> {
        (self.unsafe_get()).cellspacing.get()
    }

    fn get_width(self) -> LengthOrPercentageOrAuto {
        self.upcast::<Element>()
            .get_attr_for_layout(&ns!(), &local_name!("width"))
            .map(AttrValue::as_dimension)
            .cloned()
            .unwrap_or(LengthOrPercentageOrAuto::Auto)
    }
}

impl VirtualMethods for HTMLTableElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<HTMLElement>() as &dyn VirtualMethods)
    }

    fn attribute_mutated(&self, attr: &Attr, mutation: AttributeMutation) {
        self.super_type().unwrap().attribute_mutated(attr, mutation);
        match *attr.local_name() {
            local_name!("border") => {
                // According to HTML5 § 14.3.9, invalid values map to 1px.
                self.border.set(
                    mutation
                        .new_value(attr)
                        .map(|value| parse_unsigned_integer(value.chars()).unwrap_or(1)),
                );
            },
            local_name!("cellpadding") => {
                self.cellpadding.set(
                    mutation
                        .new_value(attr)
                        .and_then(|value| parse_unsigned_integer(value.chars()).ok()),
                );
            },
            local_name!("cellspacing") => {
                self.cellspacing.set(
                    mutation
                        .new_value(attr)
                        .and_then(|value| parse_unsigned_integer(value.chars()).ok()),
                );
            },
            _ => {},
        }
    }

    fn parse_plain_attribute(&self, local_name: &LocalName, value: DOMString) -> AttrValue {
        match *local_name {
            local_name!("border") => AttrValue::from_u32(value.into(), 1),
            local_name!("width") => AttrValue::from_nonzero_dimension(value.into()),
            local_name!("bgcolor") => AttrValue::from_legacy_color(value.into()),
            _ => self
                .super_type()
                .unwrap()
                .parse_plain_attribute(local_name, value),
        }
    }
}
