use crate::editor::prelude::*;

pub enum UndoRedo {
    Undo,
    Redo,
}

type X = usize;
type Y = usize;

#[derive(Debug, Clone)]
pub enum EditDiff {
    InsertChar(X, Y, char),
    DeleteChar(X, Y, char, bool), // Backspace or delete

    Compound(Vec<EditDiff>),

    NewLine(Y),
    DeleteLine(Y, String, LineDeleteMode),

    SplitLine(X, Y),
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum LineDeleteMode {
    Joined,
    WholeLine,
}

impl EditDiff {
    pub fn apply(&self, which: UndoRedo, doc: &mut TextDocument) -> (X, Y) {
        use EditDiff::*;
        use UndoRedo::*;

        let rows = &mut doc.rows;

        match *self {
            InsertChar(x, y, c) => match which {
                Undo => {
                    rows[y].remove_at(x);
                    (x, y)
                }
                Redo => {
                    rows[y].insert_char(x, c);
                    (x + 1, y)
                }
            },
            DeleteChar(x, y, c, is_backspace) => match which {
                Undo => {
                    rows[y].insert_char(x - 1, c);
                    (x - if is_backspace { 0 } else { 1 }, y)
                }
                Redo => {
                    rows[y].remove_at(x - 1);
                    (x - 1, y)
                }
            },

            Compound(ref d) => match which {
                Undo => {
                    let mut x = 0;
                    let mut y = 0;

                    let mut diffs = d.clone();

                    while diffs.len() > 0 {
                        let diff = diffs.pop().unwrap();
                        let a = diff.apply(Undo, doc);
                        x = a.0;
                        y = a.1;
                    }

                    (x, y)
                }
                Redo => {
                    let mut x = 0;
                    let mut y = 0;

                    for diff in d {
                        let a = diff.apply(Redo, doc);
                        x = a.0;
                        y = a.1;
                    }

                    (x, y)
                }
            },

            DeleteLine(y, ref s, mode) => match which {
                Undo => {
                    if y != 0 && mode == LineDeleteMode::Joined {
                        let l = rows[y - 1].buf.len();
                        rows[y - 1].buf.truncate(l - s.len());
                    }

                    rows.insert(y, Row::from_string(s.clone()));
                    (0, y)
                }
                Redo => {
                    rows.remove(y);
                    (rows[y - 1].len(), y - 1)
                }
            },
            NewLine(y) => match which {
                Undo => {
                    let mut cx = rows[y - 1].len();
                    let mut cy = y - 1;
                    if rows[y].len() == 0 && y + 1 < rows.len() {
                        cy = y;
                        cx = 0;
                    }

                    rows.remove(y);
                    (cx, cy)
                }
                Redo => {
                    rows.insert(y, Row::empty());
                    (0, y)
                }
            },

            SplitLine(x, y) => match which {
                Undo => {
                    let col = rows[y].len();
                    rows[y] = Row::from_string(format!("{}{}", rows[y].buf, rows[y + 1].buf));
                    rows.remove(y + 1);

                    (col, y)
                }
                Redo => {
                    let (left, right) = rows[y].split_at(x);
                    rows[y] = Row::from_string(left);
                    rows.insert(y + 1, Row::from_string(right));

                    (0, y + 1)
                }
            },
        }
    }
}
