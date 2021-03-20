use crate::editor::Row;

pub enum UndoRedo {
    Undo,
    Redo
}

type X = usize;
type Y = usize;

#[derive(Debug)]
pub enum EditDiff {
    InsertChar(X, Y, char),
    DeleteChar(X, Y, char, bool),// Backspace or delete

    NewLine(Y),
    DeleteLine(Y, String),

    SplitLine(X, Y)
}

impl EditDiff {
    pub fn apply(&self, rows: &mut Vec<Row>, which: UndoRedo) -> (X, Y) {
        use EditDiff::*;
        use UndoRedo::*;

        match *self {
            InsertChar(x, y, c) => match which {
                Undo => {
                    rows[y].remove_at(x);
                    (x,y)
                },
                Redo => {
                    rows[y].insert_char(x, c);
                    (x + 1, y)
                }
            },
            DeleteChar(x, y, c, is_backspace) => match which {
                Undo => {
                    rows[y].insert_char(x - 1, c);
                    (x - if is_backspace { 0 } else { 1 }, y)
                },
                Redo => {
                    rows[y].remove_at(x - 1);
                    (x - 1, y)
                }
            },

            
            DeleteLine(y, ref s) => match which {
                Undo => {
                    if y != 0 {
                        let l = rows[y - 1].buf.len();
                        rows[y - 1].buf.truncate(l - s.len());
                    }
                    
                    rows.insert(y, Row::from_string(s.clone()));
                    (0,y)
                },
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
                },
                Redo => {
                    rows.insert(y, Row::empty());
                    (0, y)
                }
            },

            SplitLine(x, y) => match which {
                Undo => {
                    let col = rows[y].len();
                    rows[y] = Row::from_string(format!("{}{}",rows[y].buf,rows[y + 1].buf));
                    rows.remove(y + 1);

                    (col, y)
                },
                Redo => {
                    let (left, right) = rows[y].split_at(x);
                    rows[y] = Row::from_string(left);
                    rows.insert(y + 1, Row::from_string(right));

                    (0, y + 1)
                }
            }
        }
    }
}