use reformation::Reformation;
use std::collections::VecDeque;

#[derive(PartialEq,Eq,Hash,Debug,Reformation,Copy,Clone)]
enum Op {
    #[reformation("swap position {} with position {}")]
    SwapPos(usize,usize),
    #[reformation("swap letter {} with letter {}")]
    SwapLet(char,char),
    #[reformation("rotate left {} steps?")]
    RotateLeft(usize),
    #[reformation("rotate right {} steps?")]
    RotateRight(usize),
    #[reformation("rotate based on position of letter {}")]
    RotateOnCharPos(char),
    #[reformation("reverse positions {} through {}")]
    ReverseRange(usize,usize),
    #[reformation("move position {} to position {}")]
    Move(usize,usize)
}

impl Op {
    fn unapply(self, arr: &mut VecDeque<char>) {
        match self {
            Op::SwapPos(a, b) => arr.swap(a,b),
            Op::SwapLet(a, b) => {
                let a_pos = arr.iter().position(|&x| x == a).expect("can't find a");
                let b_pos = arr.iter().position(|&x| x == b).expect("can't find b");
                arr.swap(a_pos,b_pos);
            },
            Op::RotateLeft(a) => {
                arr.rotate_right(a);
            },
            Op::RotateRight(a) => {arr.rotate_left(a);},
            Op::RotateOnCharPos(c) => {
                //before, func was:
                //0 -> 1 (and became pos 1)
                //1 -> 2 (3)
                //2 -> 3 (5)
                //3 -> 4 (7)
                //4 -> 6 (2)
                //5 -> 7 (4)
                //6 -> 8 (6)
                //7 -> 1 (0)
                //so reverse func, new pos -> lrot = 0: 1, 1: 1, 2: 6, 3: 2, 4: 7, 5: 3, 6: 8, 7: 4!
                let a_pos = arr.iter().position(|&x| x == c).expect("can't find c");
                let dist = [1,1,6,2,7,3,8,4][a_pos];
                arr.rotate_left(dist);
            },
            Op::ReverseRange(a, b) => {
                for x in 0..(b-a+1)/2 {
                    arr.swap(a+x,b-x);
                }
            },
            Op::Move(y, x) => {
                if x < y {
                    let orig = arr[x];
                    for p in x..y {
                        arr[p] = arr[p+1];
                    }
                    arr[y] = orig;
                } else {
                    let orig = arr[x];
                    for p in (y+1..=x).rev() {
                        arr[p] = arr[p-1];
                    }
                    arr[y] = orig;
                }
            },
        }
    }
    fn apply(self, arr: &mut VecDeque<char>) {
        match self {
            Op::SwapPos(a, b) => arr.swap(a,b),
            Op::SwapLet(a, b) => {
                let a_pos = arr.iter().position(|&x| x == a).expect("can't find a");
                let b_pos = arr.iter().position(|&x| x == b).expect("can't find b");
                arr.swap(a_pos,b_pos);
            },
            Op::RotateLeft(a) => {
                arr.rotate_left(a);
            },
            Op::RotateRight(a) => {arr.rotate_right(a);},
            Op::RotateOnCharPos(c) => {
                let a_pos = arr.iter().position(|&x| x == c).expect("can't find c");
                let dist = (if a_pos >= 4 {a_pos + 2} else {a_pos + 1}) % arr.len();
                arr.rotate_right(dist);
            },
            Op::ReverseRange(a, b) => {
                for x in 0..(b-a+1)/2 {
                    arr.swap(a+x,b-x);
                }
            },
            Op::Move(x, y) => {
                if x < y {
                    let orig = arr[x];
                    for p in x..y {
                        arr[p] = arr[p+1];
                    }
                    arr[y] = orig;
                } else {
                    let orig = arr[x];
                    for p in (y+1..=x).rev() {
                        arr[p] = arr[p-1];
                    }
                    arr[y] = orig;
                }
            },
        }
    }
}

#[aoc(day21,part1)]
#[post(ret == "agcebfdh")]
fn p1(input: &str) -> String {
    let steps = input.lines().map(|x| Op::parse(x).unwrap());
    let mut s : VecDeque<char> = "abcdefgh".chars().collect();
    for step in steps{
        step.apply(&mut s);
    }
    s.iter().collect()
}

#[aoc(day21,part2)]
#[post(ret == "afhdbegc")]
fn p2(input: &str) -> String {
    let steps = input.lines().map(|x| Op::parse(x).unwrap());
    let mut s : VecDeque<char> = "fbgdceah".chars().collect();
    for step in steps.rev(){
        step.unapply(&mut s);
    }
    s.iter().collect()
}
