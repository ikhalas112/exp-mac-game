use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const BOARD_WIDTH: i32 = 20;
pub const BOARD_HEIGHT: i32 = 20;
pub const POINTS_PER_FOOD: u32 = 10;
pub const MAX_HIGH_SCORES: usize = 10;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub fn is_opposite(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Up, Self::Down)
                | (Self::Down, Self::Up)
                | (Self::Left, Self::Right)
                | (Self::Right, Self::Left)
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameStatus {
    Ready,
    Running,
    GameOver,
}

#[derive(Clone, Debug)]
pub struct GameState {
    pub snake: Vec<Point>,
    pub food: Point,
    pub direction: Direction,
    pub pending_direction: Direction,
    pub score: u32,
    pub status: GameStatus,
}

impl Default for GameState {
    fn default() -> Self {
        let center = Point::new(BOARD_WIDTH / 2, BOARD_HEIGHT / 2);
        let snake = vec![
            center,
            Point::new(center.x - 1, center.y),
            Point::new(center.x - 2, center.y),
        ];
        let mut state = Self {
            snake,
            food: Point::new(center.x + 4, center.y),
            direction: Direction::Right,
            pending_direction: Direction::Right,
            score: 0,
            status: GameStatus::Ready,
        };
        state.food = state.random_food();
        state
    }
}

impl GameState {
    pub fn start(&mut self) {
        if self.status != GameStatus::Running {
            self.status = GameStatus::Running;
        }
    }

    pub fn restart(&mut self) {
        *self = Self::default();
        self.status = GameStatus::Running;
    }

    pub fn set_direction(&mut self, direction: Direction) {
        if !self.direction.is_opposite(direction) {
            self.pending_direction = direction;
        }
    }

    pub fn tick(&mut self) {
        if self.status != GameStatus::Running {
            return;
        }

        self.direction = self.pending_direction;
        let mut next_head = self.snake[0];
        match self.direction {
            Direction::Up => next_head.y -= 1,
            Direction::Down => next_head.y += 1,
            Direction::Left => next_head.x -= 1,
            Direction::Right => next_head.x += 1,
        }

        if next_head.x < 0
            || next_head.y < 0
            || next_head.x >= BOARD_WIDTH
            || next_head.y >= BOARD_HEIGHT
            || self.snake.contains(&next_head)
        {
            self.status = GameStatus::GameOver;
            return;
        }

        self.snake.insert(0, next_head);
        if next_head == self.food {
            self.score += POINTS_PER_FOOD;
            self.food = self.random_food();
        } else {
            self.snake.pop();
        }
    }

    pub fn cells(&self) -> Vec<Cell> {
        let mut cells = Vec::with_capacity((BOARD_WIDTH * BOARD_HEIGHT) as usize);
        for y in 0..BOARD_HEIGHT {
            for x in 0..BOARD_WIDTH {
                let point = Point::new(x, y);
                let kind = if self.snake.first() == Some(&point) {
                    CellKind::Head
                } else if self.snake.contains(&point) {
                    CellKind::Body
                } else if self.food == point {
                    CellKind::Food
                } else {
                    CellKind::Empty
                };
                cells.push(Cell { point, kind });
            }
        }
        cells
    }

    fn random_food(&self) -> Point {
        let mut rng = rand::rng();
        loop {
            let point = Point::new(
                rng.random_range(0..BOARD_WIDTH),
                rng.random_range(0..BOARD_HEIGHT),
            );
            if !self.snake.contains(&point) {
                return point;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cell {
    pub point: Point,
    pub kind: CellKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellKind {
    Empty,
    Head,
    Body,
    Food,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GetOrCreateUserRequest {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UserResponse {
    pub user: User,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LeaderboardEntry {
    pub user_id: Uuid,
    pub player_name: String,
    pub score: u32,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SubmitScoreRequest {
    pub user_id: Uuid,
    pub score: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScoresResponse {
    pub scores: Vec<LeaderboardEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_immediate_reverse_direction() {
        let mut state = GameState::default();
        state.set_direction(Direction::Left);
        assert_eq!(state.pending_direction, Direction::Right);
    }

    #[test]
    fn ticks_forward() {
        let mut state = GameState::default();
        state.start();
        let head = state.snake[0];
        state.tick();
        assert_eq!(state.snake[0], Point::new(head.x + 1, head.y));
    }

    #[test]
    fn eating_food_increases_score_and_length() {
        let mut state = GameState::default();
        state.start();
        let head = state.snake[0];
        state.food = Point::new(head.x + 1, head.y);
        let length = state.snake.len();
        state.tick();
        assert_eq!(state.score, POINTS_PER_FOOD);
        assert_eq!(state.snake.len(), length + 1);
    }

    #[test]
    fn wall_collision_ends_game() {
        let mut state = GameState::default();
        state.start();
        state.snake[0] = Point::new(BOARD_WIDTH - 1, state.snake[0].y);
        state.tick();
        assert_eq!(state.status, GameStatus::GameOver);
    }
}
