use std::marker::PhantomData;

// this module adds some functionality based on the required implementations
// here like: `LinkedList::pop_back` or `Clone for LinkedList<T>`
// You are free to use anything in it, but it's mainly for the test framework.
mod pre_implemented;

pub struct LinkedList<T> {
    len: usize,
    head: Option<*mut Node<T>>,
    tail: Option<*mut Node<T>>,
}

pub struct Node<T> {
    value: T,
    next: Option<*mut Node<T>>,
    prev: Option<*mut Node<T>>,
}

impl<T> Node<T> {
    fn new(value: T) -> Node<T> {
        Node {
            value,
            next: None,
            prev: None,
        }
    }

    // This will be called only on a valid and existing node.
    // The result of this is that the optionally new nodes and/or previous nodes will be linked
    // ensuring double-links prev <--> new_node <--> next depending on cursor positions.
    unsafe fn link_nodes(&mut self, next: Option<*mut Node<T>>, prev: Option<*mut Node<T>>) {
        self.link_next(next);
        self.link_prev(prev)
    }

    unsafe fn link_next(&mut self, next: Option<*mut Node<T>>) {
        self.next = next;
    }

    unsafe fn link_prev(&mut self, prev: Option<*mut Node<T>>) {
        self.prev = prev;
    }
}

pub struct Cursor<'a, T> {
    node: Option<*mut Node<T>>,
    list: &'a mut LinkedList<T>,
}

impl<T> Default for LinkedList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> LinkedList<T> {
    pub fn new() -> Self {
        LinkedList {
            len: 0,
            head: None,
            tail: None,
        }
    }

    // You may be wondering why it's necessary to have is_empty()
    // when it can easily be determined from len().
    // It's good custom to have both because len() can be expensive for some types,
    // whereas is_empty() is almost always cheap.
    // (Also ask yourself whether len() is expensive for LinkedList)
    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    /// Return a cursor positioned on the front element
    pub fn cursor_front(&mut self) -> Cursor<T> {
        Cursor {
            node: if self.head.is_some() {
                Some(self.head.unwrap())
            } else {
                None
            },
            list: self,
        }
    }

    /// Return a cursor positioned on the back element
    pub fn cursor_back(&mut self) -> Cursor<T> {
        Cursor {
            node: if self.tail.is_some() {
                Some(self.tail.unwrap())
            } else {
                None
            },
            list: self,
        }
    }

    /// Return an iterator that moves from front to back
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            next: self.head,
            marker: PhantomData,
        }
    }
}

// the cursor is expected to act as if it is at the position of an element
// and it also has to work with and be able to insert into an empty list.
impl<'a, T> Cursor<'a, T> {
    /// Take a mutable reference to the current element
    pub fn peek_mut(&mut self) -> Option<&mut T> {
        // The pointer does not get dereferenced unless the node exists.
        unsafe { Some(&mut (*self.node?).value) }
    }

    /// Move one position forward (towards the back) and
    /// return a reference to the new position
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&mut T> {
        match self.node.take() {
            Some(current) => unsafe {
                // We shift the pointer to the next node if it is present.
                self.node = (*current).next;
                self.peek_mut()
            },
            None => {
                self.node = self.list.head;
                None
            }
        }
    }

    /// Move one position backward (towards the front) and
    /// return a reference to the new position
    pub fn prev(&mut self) -> Option<&mut T> {
        match self.node.take() {
            Some(current) => unsafe {
                // We shift the pointer to the previous node if it is present.
                self.node = (*current).prev;
                self.peek_mut()
            },
            None => {
                self.node = self.list.tail;
                None
            }
        }
    }

    /// Remove and return the element at the current position and move the cursor
    /// to the neighboring element that's closest to the back. This can be
    /// either the next or previous position.
    pub fn take(&mut self) -> Option<T> {
        unsafe {
            // If the node is None we immediately short circuit out.
            let node = self.node?;

            // When the node exists it is moved de-allocating memory from the pointer.
            let moved_node = std::boxed::Box::from_raw(node);

            // The next and prev of the moved nodes get disconnected...
            let next = moved_node.next;
            let prev = moved_node.prev;

            self.node = next.or(prev);

            // ... and then get reconnected accordingly.
            match next {
                Some(mut next) => (*next).prev = prev,
                None => self.list.tail = prev,
            };

            match prev {
                Some(mut prev) => (*prev).next = next,
                None => self.list.head = next,
            };

            self.list.len -= 1;

            Some(moved_node.value)
        }
    }

    // Inserting a new node after the cursor requires
    // ... <--> cursor <--> new_node. The previously next node
    // of the cursor gets its prev pointer pointing at the new node.
    pub fn insert_after(&mut self, element: T) {
        unsafe {
            // If the cursor node does not exist, it is an empty list
            // so we insert the first node and return.
            let Some(cursor_node) = self.node else  {
                self.insert_first(element);
                return;
            };

            // Unwrap is asserted to return `Some(&node)` from the cursor node.
            let &Node {
                value: _,
                next,
                prev: _,
            } = cursor_node.as_ref().unwrap();

            // The new node is created on the heap and is linked to
            // its prev and next nodes.
            let mut ptr = Box::new(Node::new(element));
            ptr.as_mut().link_nodes(next, Some(cursor_node));

            let new_node = std::boxed::Box::<Node<T>>::into_raw(ptr);

            // The new node is inserted after the cursor so it becomes the cursor's `next`.
            (*cursor_node).next = Some(new_node);

            // The cursor's former next node becomes linked to the new node,
            // with the new node being the prev.
            if let Some(next_node) = next {
                (*next_node).link_prev(Some(new_node));
            }

            // When insert_after is called while the cursor is at the tail,
            // this means we are adding a new node to the end of the list and so
            // it becomes the tail.
            if Some(cursor_node) == self.list.tail {
                self.list.tail = Some(new_node);
            }

            self.list.len += 1;
        }
    }

    // Inserting a new node before the cursor requires
    // new_node <--> cursor <--> .... The previously previous node
    // of the cursor gets its next pointer pointing at the new node.
    pub fn insert_before(&mut self, element: T) {
        unsafe {
            // If the cursor node does not exist, it is an empty list
            // so we insert the first node and return.
            let Some(cursor_node) = self.node else  {
                self.insert_first(element);
                return;
            };

            // Unwrap is asserted to return `Some(&node)` from the cursor node.
            let &Node {
                value: _,
                next: _,
                prev,
            } = cursor_node.as_ref().unwrap();

            // The new node is created on the heap and is linked to
            // its prev and next nodes.
            let mut ptr = Box::new(Node::new(element));
            ptr.as_mut().link_nodes(Some(cursor_node), prev);

            let new_node = std::boxed::Box::<Node<T>>::into_raw(ptr);

            // The new node is inserted before the cursor so it becomes the cursor's `prev`.
            (*cursor_node).prev = Some(new_node);

            // The cursor's former prev node becomes linked to the new node,
            // with the new node being the next.
            if let Some(prev_node) = prev {
                (*prev_node).link_next(Some(new_node));
            }

            // When insert_before is called while the cursor is at the head,
            // this means we are adding a new node to the beginning of the list and so
            // it becomes the head.
            if Some(cursor_node) == self.list.head {
                self.list.head = Some(new_node);
            }

            self.list.len += 1;
        }
    }

    // This creates the first node in the linked list. Memory is always heap allocated and the
    // pointer is then returned. The first node is naturally the head and tail of the list.
    fn insert_first(&mut self, element: T) -> *mut Node<T> {
        let new_node = Node::new(element);
        let node_ptr = std::boxed::Box::<Node<T>>::into_raw(Box::new(new_node));
        self.node = Some(node_ptr);
        self.list.head = Some(node_ptr);
        self.list.tail = Some(node_ptr);
        self.list.len += 1;

        node_ptr
    }
}

pub struct Iter<'a, T> {
    next: Option<*mut Node<T>>,
    marker: PhantomData<&'a LinkedList<T>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        // The iterator will continue to move forward
        // so long as next points to an existing node.
        // It will short-circuit once it hits the first `None`.
        let next_node = self.next?;

        unsafe {
            let node = &(*next_node);

            self.next = node.next;

            Some(&node.value)
        }
    }
}

impl<T> Drop for LinkedList<T> {
    fn drop(&mut self) {
        let mut cursor = self.cursor_front();
        while cursor.take().is_some() {}
    }
}
