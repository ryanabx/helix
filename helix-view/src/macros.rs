//! These are macros to make getting very nested fields in the `Editor` struct easier
//! These are macros instead of functions because functions will have to take `&mut self`
//! However, rust doesn't know that you only want a partial borrow instead of borrowing the
//! entire struct which `&mut self` says.  This makes it impossible to do other mutable
//! stuff to the struct because it is already borrowed. Because macros are expanded,
//! this circumvents the problem because it is just like indexing fields by hand and then
//! putting a `&mut` in front of it. This way rust can see that we are only borrowing a
//! part of the struct and not the entire thing.

/// Get the current view and document mutably as a tuple.
/// Returns `(&mut View, &mut Document)`
#[macro_export]
macro_rules! current {
    ($editor:expr) => {{
        if let Some(view) = $crate::view_mut_doc!($editor)
        {
            let id = view.doc;
            if let Some(doc) = $crate::doc_mut!($editor, &id)
            {
                Some((view, doc))
            }
            else {
                None
            }
        }
        else {
            None
        }
    }};
}

#[macro_export]
macro_rules! current_ref_doc {
    ($editor:expr) => {{
        let view = $editor.tree.get($editor.tree.focus);
        if let View::Document(view) = view {
            let doc = &$editor.documents[&view.doc];
            Some((view, doc))
        } else {
            None
        }
    }};
}

/// Get the current document mutably.
/// Returns `&mut Document`
#[macro_export]
macro_rules! doc_mut {
    ($editor:expr, $id:expr) => {{
        Some($editor.documents.get_mut($id).unwrap())
    }};
    ($editor:expr) => {{
        if let Some((_, doc)) = $crate::current!($editor) {
            Some(doc)
        }
        else {
            None
        }
    }};
}

/// Get the current document view mutably, if it exists.
/// Returns `&mut DocumentView`
#[macro_export]
macro_rules! view_mut_doc {
    ($editor:expr, $id:expr) => {{
        let view = $editor.tree.get_mut($id);
        if let View::Document(view) = view {
            Some(view)
        }
        else {
            None
        }
    }};
    ($editor:expr) => {{
        let view = $editor.tree.get_mut($editor.tree.focus);
        if let View::Document(view) = view {
            Some(view)
        }
        else {
            None
        }
    }};
}

/// Get the current view immutably
/// Returns `&View`
#[macro_export]
macro_rules! view {
    ($editor:expr, $id:expr) => {{
        $editor.tree.get($id)
    }};
    ($editor:expr) => {{
        $editor.tree.get($editor.tree.focus)
    }};
}

#[macro_export]
macro_rules! doc {
    ($editor:expr, $id:expr) => {{
        &$editor.documents[$id]
    }};
    ($editor:expr) => {{
        if let Some((_, doc)) = $crate::current_ref_doc!($editor) {
            Some(doc)
        }
        else {
            None
        }
    }};
}
