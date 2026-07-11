use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::{TiledColliderOf, TiledObject};

use crate::{
    level::{Layer, LocationBoundary},
    movement::Ground,
    object::Object,
};

/// Квантизация по XY. Чем меньше, тем точнее, но больше вычислений.
const CELL_SIZE: f32 = 100.0;
/// Множитель для смены слоя. Чем выше, тем реже AI будет скакать по слоям.
/// 1 – без разницы идти по X или Z; 0 – смена слоя запрещена.
const LAYER_CHANGE_COST: i32 = 2;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<NavGrid>();
    app.add_observer(init);
    app.add_systems(FixedUpdate, on_collider_change);
}

/// Сегмент пути.
#[derive(Debug, Clone)]
pub struct PathSegment {
    pub position: Vec2,
    pub layer: Layer,
}

/// 3D карта физики для расчёта поиска пути.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct NavGrid {
    anchor: Vec2,
    width: usize,
    height: usize,
    layers: usize,
    walkable: Vec<bool>,
    blocked: Vec<bool>,
}

impl NavGrid {
    fn new(boundary: &LocationBoundary, anchor: Vec2) -> Self {
        let rect = boundary.to_rect();
        let width = (rect.width() as f32 / CELL_SIZE).ceil() as usize;
        let height = (rect.height() as f32 / CELL_SIZE).ceil() as usize;
        let layers = boundary.layers();

        Self {
            anchor,
            width,
            height,
            layers,
            walkable: vec![false; width * height * layers],
            blocked: vec![false; width * height * layers],
        }
    }

    fn compute_walkable(
        &mut self,
        collider_q: &Query<(
            &Collider,
            &GlobalTransform,
            &Layer,
            Option<&TiledColliderOf>,
        )>,
        ground_q: &Query<(), With<Ground>>,
    ) {
        for (collider, global_transform, layer, tiled_collider_of) in collider_q {
            if !is_ground_collider(tiled_collider_of, ground_q) {
                continue;
            }

            let layer_id = layer.id() as usize - 1;

            if layer_id >= self.layers {
                continue;
            }

            let aabb = collider.aabb(
                global_transform.translation().truncate(),
                global_transform.rotation(),
            );

            let rect = Rect::from_corners(aabb.min, aabb.max);
            self.add_ground(rect, layer_id);
        }
    }

    fn add_ground(&mut self, rect: Rect, layer_id: usize) {
        if layer_id >= self.layers {
            return;
        }

        let (x0, y0) = self.world_to_cell_limited(rect.min);
        let (x1, y1) = self.world_to_cell_limited(rect.max);

        for y in y0..=y1 {
            for x in x0..=x1 {
                let idx = self.index(x, y, layer_id);
                self.walkable[idx] = true;
            }
        }
    }

    fn add_obstacle(&mut self, rect: Rect, layer: &Layer) {
        let layer_id = layer.id() as usize - 1;

        if layer_id >= self.layers {
            error!("Layer {} is out of bounds", layer.id());
            return;
        }

        let (x0, y0) = self.world_to_cell_limited(rect.min);
        let (x1, y1) = self.world_to_cell_limited(rect.max);

        if x0 > x1 || y0 > y1 {
            return;
        }

        for y in y0..=y1 {
            for x in x0..=x1 {
                let idx = self.index(x, y, layer_id);
                self.blocked[idx] = true;
            }
        }
    }

    fn reset(&mut self) {
        self.walkable.fill(false);
        self.blocked.fill(false);
    }

    fn count_layer(cells: &[bool], width: usize, height: usize, layer: usize) -> usize {
        let start = layer * width * height;
        cells[start..start + width * height]
            .iter()
            .filter(|cell| **cell)
            .count()
    }

    #[cfg(test)]
    pub(crate) fn test_grid(width: usize, height: usize, layers: usize) -> Self {
        Self {
            width,
            height,
            layers,
            anchor: Vec2::ZERO,
            walkable: vec![false; width * height * layers],
            blocked: vec![false; width * height * layers],
        }
    }

    #[cfg(test)]
    pub(crate) fn test_fill_walkable(&mut self) {
        self.walkable.fill(true);
    }

    #[cfg(test)]
    pub(crate) fn test_add_ground(&mut self, rect: Rect, layer: Layer) {
        self.add_ground(rect, layer.id() as usize - 1);
    }

    #[cfg(test)]
    pub(crate) fn test_count_walkable(&self, layer: Layer) -> usize {
        Self::count_layer(
            &self.walkable,
            self.width,
            self.height,
            layer.id() as usize - 1,
        )
    }

    #[cfg(test)]
    pub(crate) fn test_count_blocked(&self, layer: Layer) -> usize {
        Self::count_layer(
            &self.blocked,
            self.width,
            self.height,
            layer.id() as usize - 1,
        )
    }

    /// Ищет путь от точки `start_world` на слое `start_layer` до точки `target_world` на слое
    /// `target_layer`. Точки задаются в глобальных координатах. `search_limit` ограничивает
    /// поиск по кол-ву рассмотренных вариантов.
    ///
    /// Реализует алгоритм A* поиска пути на сетке с учётом слоёв.
    pub fn find_path(
        &self,
        start_world: Vec2,
        start_layer: Layer,
        target_world: Vec2,
        target_layer: Layer,
        search_limit: usize,
    ) -> Option<Vec<PathSegment>> {
        // Перевод координат.
        let Some(start) = self.build_node(start_world, start_layer) else {
            return None;
        };

        let Some(target) = self.build_node(target_world, target_layer) else {
            return None;
        };

        // Проверка границ.
        if start.x >= self.width || target.x >= self.width {
            return None;
        }

        let mut open = BinaryHeap::new();
        let mut came_from: HashMap<Node, Node> = HashMap::new();
        let mut g_score: HashMap<Node, i32> = HashMap::new();
        g_score.insert(start, 0);

        open.push(OpenNode {
            node: start,
            f: Self::heuristic(start, target),
        });

        let mut visited = 0;

        while let Some(OpenNode { node, .. }) = open.pop() {
            visited += 1;

            if visited > search_limit {
                return None;
            }

            if node.x == target.x && node.layer_id == target.layer_id {
                // Нашли вариант, собираем путь.
                let mut path = vec![node];
                let mut current = node;

                while let Some(prev) = came_from.get(&current) {
                    path.push(*prev);
                    current = *prev;
                }

                path.reverse();

                let result: Vec<_> = path
                    .into_iter()
                    .map(|n| PathSegment {
                        position: self.cell_to_world(n.x, n.y),
                        layer: Layer::new(n.layer_id as u32 + 1),
                    })
                    .collect();

                return Some(result);
            }

            for (next, step_cost) in self.neighbors(node) {
                let tentative_g = g_score.get(&node).expect("g_score") + step_cost;

                if tentative_g < *g_score.get(&next).unwrap_or(&i32::MAX) {
                    came_from.insert(next, node);
                    g_score.insert(next, tentative_g);
                    let f = tentative_g + Self::heuristic(next, target);
                    open.push(OpenNode { node: next, f });
                }
            }
        }

        None
    }

    fn build_node(&self, coordinate: Vec2, layer: Layer) -> Option<Node> {
        let (x, y) = self.world_to_cell(coordinate);
        let layer_id = layer.id() as usize - 1;

        if x >= self.width || layer_id >= self.layers {
            return None;
        }

        self.nearest_standable_y(x, y.min(self.height - 1), layer_id)
            .map(|y| Node { x, y, layer_id })
    }

    fn neighbors(&self, node: Node) -> Vec<(Node, i32)> {
        let mut neighbors = Vec::with_capacity(4);

        for dx in [-1, 1] {
            let nx = node.x as isize + dx;

            if !(0..self.width as isize).contains(&nx) {
                continue;
            }

            if let Some(y) = self.nearest_standable_y(nx as usize, node.y, node.layer_id) {
                neighbors.push((
                    Node {
                        x: nx as usize,
                        y,
                        layer_id: node.layer_id,
                    },
                    1,
                ));
            }
        }

        for dl in [-1, 1] {
            let layer_id = node.layer_id as isize + dl;

            if !(0..self.layers as isize).contains(&layer_id) {
                continue;
            }

            if let Some(y) = self.nearest_standable_y(node.x, node.y, layer_id as usize) {
                neighbors.push((
                    Node {
                        x: node.x,
                        y,
                        layer_id: layer_id as usize,
                    },
                    LAYER_CHANGE_COST,
                ));
            }
        }

        neighbors
    }

    fn nearest_standable_y(&self, x: usize, y: usize, layer_id: usize) -> Option<usize> {
        (0..self.height)
            .filter_map(|candidate_y| {
                let node = Node {
                    x,
                    y: candidate_y,
                    layer_id,
                };

                (self.is_walkable(&node) && !self.is_blocked(&node))
                    .then_some((candidate_y, (candidate_y as i32 - y as i32).unsigned_abs()))
            })
            .min_by_key(|(candidate_y, distance)| (*distance, *candidate_y))
            .map(|(candidate_y, _)| candidate_y)
    }

    #[inline]
    fn index(&self, x: usize, y: usize, layer: usize) -> usize {
        layer * self.width * self.height + y * self.width + x
    }

    #[inline]
    fn is_blocked(&self, node: &Node) -> bool {
        self.blocked[self.index(node.x, node.y, node.layer_id)]
    }

    #[inline]
    fn is_walkable(&self, node: &Node) -> bool {
        self.walkable[self.index(node.x, node.y, node.layer_id)]
    }

    #[inline]
    fn world_to_cell_limited(&self, coordinate: Vec2) -> (usize, usize) {
        let (x, y) = self.world_to_cell(coordinate);

        (
            x.clamp(0, self.width as usize - 1),
            y.clamp(0, self.height as usize - 1),
        )
    }

    #[inline]
    fn world_to_cell(&self, coordinate: Vec2) -> (usize, usize) {
        let diff = coordinate - self.anchor;

        (
            (diff.x / CELL_SIZE).floor() as usize,
            (diff.y / CELL_SIZE).floor() as usize,
        )
    }

    #[inline]
    fn cell_to_world(&self, x: usize, y: usize) -> Vec2 {
        let x = (x as f32 + 0.5) * CELL_SIZE;
        let y = (y as f32 + 0.5) * CELL_SIZE;
        Vec2::new(x, y) + self.anchor
    }

    /// Манхэттенское расстояние от `a` до `b` для оценки скора маршрута.
    fn heuristic(a: Node, b: Node) -> i32 {
        (a.x as i32 - b.x as i32).abs()
            + (a.layer_id as i32 - b.layer_id as i32).abs() * LAYER_CHANGE_COST
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct Node {
    x: usize,
    y: usize,
    layer_id: usize,
}

#[derive(Copy, Clone, Debug)]
struct OpenNode {
    node: Node,
    f: i32,
}

impl PartialEq for OpenNode {
    fn eq(&self, other: &Self) -> bool {
        self.f == other.f
    }
}

impl Eq for OpenNode {}

impl PartialOrd for OpenNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OpenNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f.cmp(&self.f)
    }
}

fn init(
    inserted: On<Insert, LocationBoundary>,
    q: Query<(&LocationBoundary, &GlobalTransform)>,
    mut commands: Commands,
) {
    let Ok((location_boundary, global_transform)) = q.get(inserted.entity) else {
        return;
    };

    let grid = NavGrid::new(location_boundary, global_transform.translation().truncate());
    commands.entity(inserted.entity).insert(grid);
}

fn on_collider_change(
    mut grid_q: Query<&mut NavGrid>,
    added_q: Query<(), Added<Collider>>,
    changed_transform_q: Query<
        (Entity, &Transform),
        (
            With<Collider>,
            // FIXME: Непонятно почему пол постоянно триггерится на Changed<Transform>. Костыль.
            With<TiledObject>,
            Changed<Transform>,
        ),
    >,
    changed_collider_q: Query<(), (With<Collider>, Changed<Collider>)>,
    mut removed_q: RemovedComponents<Collider>,
    collider_q: Query<(
        &Collider,
        &GlobalTransform,
        &Layer,
        Option<&TiledColliderOf>,
    )>,
    ground_q: Query<(), With<Ground>>,
    obstacles_q: Query<
        (
            &Collider,
            &GlobalTransform,
            &Layer,
            Option<&TiledColliderOf>,
        ),
        With<Object>,
    >,
) {
    let Ok(mut grid) = grid_q.single_mut() else {
        return;
    };

    // Если ничего не поменялось, ничего не делаем.
    if added_q.is_empty()
        && changed_transform_q.is_empty()
        && changed_collider_q.is_empty()
        && removed_q.read().next().is_none()
    {
        return;
    }

    // Если что-то поменялось, пересчитываем карту полностью.
    // Если будет тормозить, то надо будет переделать на инкрементальные обновления.
    grid.reset();
    grid.compute_walkable(&collider_q, &ground_q);

    for (collider, global_transform, layer, tiled_collider_of) in obstacles_q {
        if is_ground_collider(tiled_collider_of, &ground_q) {
            continue;
        }

        let aabb = collider.aabb(
            global_transform.translation().truncate(),
            global_transform.rotation(),
        );

        let rect = Rect::from_corners(aabb.min, aabb.max);
        grid.add_obstacle(rect, layer);
    }
}

fn is_ground_collider(
    tiled_collider_of: Option<&TiledColliderOf>,
    ground_q: &Query<(), With<Ground>>,
) -> bool {
    tiled_collider_of
        .map(|tiled_collider_of| ground_q.get(tiled_collider_of.0).is_ok())
        .unwrap_or(false)
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn rect(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Rect {
        Rect::from_corners(Vec2::new(min_x, min_y), Vec2::new(max_x, max_y))
    }

    fn make_grid() -> NavGrid {
        let mut grid = NavGrid::test_grid(10, 10, 3);
        grid.test_fill_walkable();
        grid
    }

    fn path_is_valid(grid: &NavGrid, path: &[PathSegment]) -> bool {
        for seg in path {
            let node = Node {
                x: (seg.position.x / CELL_SIZE).floor() as usize,
                y: (seg.position.y / CELL_SIZE).floor() as usize,
                layer_id: seg.layer.id() as usize - 1,
            };

            if grid.is_blocked(&node) {
                return false;
            }
        }
        true
    }

    #[test]
    fn ground_marks_walkable_cells_only_on_its_layer() {
        let mut grid = NavGrid::test_grid(10, 10, 3);
        grid.test_add_ground(rect(0.0, 0.0, 250.0, 50.0), Layer::new(2));

        assert_eq!(grid.test_count_walkable(Layer::new(1)), 0);
        assert!(grid.test_count_walkable(Layer::new(2)) > 0);
        assert_eq!(grid.test_count_walkable(Layer::new(3)), 0);
    }

    #[test]
    fn ground_is_not_blocking_obstacle() {
        let mut grid = NavGrid::test_grid(10, 10, 3);
        grid.test_add_ground(rect(0.0, 0.0, 250.0, 50.0), Layer::new(2));

        assert!(grid.test_count_walkable(Layer::new(2)) > 0);
        assert_eq!(grid.test_count_blocked(Layer::new(2)), 0);
    }

    #[test]
    fn obstacles_block_only_their_layer() {
        let mut grid = make_grid();
        grid.add_obstacle(rect(100.0, 0.0, 200.0, 100.0), &Layer::new(2));

        assert_eq!(grid.test_count_blocked(Layer::new(1)), 0);
        assert!(grid.test_count_blocked(Layer::new(2)) > 0);
        assert_eq!(grid.test_count_blocked(Layer::new(3)), 0);
    }

    #[test]
    fn find_path_on_same_layer_moves_horizontally() {
        let grid = make_grid();
        let layer = Layer::new(1);

        let path = grid
            .find_path(
                Vec2::new(100.0, 100.0),
                layer,
                Vec2::new(700.0, 100.0),
                layer,
                1_000,
            )
            .expect("path");

        assert!(path.len() > 1);
        assert_eq!(path.last().expect("last").layer, layer);
        assert!(path.last().expect("last").position.x > path.first().expect("first").position.x);
        assert!(path.iter().all(|segment| segment.layer == layer));
    }

    #[test]
    fn find_path_between_layers_uses_intermediate_layers() {
        let grid = make_grid();

        let path = grid
            .find_path(
                Vec2::new(100.0, 100.0),
                Layer::new(3),
                Vec2::new(100.0, 100.0),
                Layer::new(1),
                1_000,
            )
            .expect("path");

        let used_layers = path.iter().map(|p| p.layer.id()).collect::<Vec<_>>();
        assert_eq!(used_layers, vec![3, 2, 1]);
    }

    #[test]
    fn find_path_avoids_blocked_intermediate_layer_cells() {
        let mut grid = make_grid();
        grid.add_obstacle(rect(0.0, 0.0, 100.0, 1_000.0), &Layer::new(2));

        let path = grid
            .find_path(
                Vec2::new(50.0, 50.0),
                Layer::new(3),
                Vec2::new(50.0, 50.0),
                Layer::new(1),
                1_000,
            )
            .expect("path");

        let layer_2_segment = path
            .iter()
            .find(|segment| segment.layer == Layer::new(2))
            .expect("layer 2 segment");

        assert!(layer_2_segment.position.x >= 150.0);
        assert!(path_is_valid(&grid, &path));
    }

    #[test]
    fn find_path_returns_none_when_no_standable_start_or_target() {
        let mut grid = NavGrid::test_grid(10, 10, 3);
        grid.test_add_ground(rect(0.0, 0.0, 200.0, 100.0), Layer::new(1));

        assert!(
            grid.find_path(
                Vec2::new(50.0, 50.0),
                Layer::new(1),
                Vec2::new(850.0, 50.0),
                Layer::new(1),
                1_000,
            )
            .is_none()
        );

        assert!(
            grid.find_path(
                Vec2::new(850.0, 50.0),
                Layer::new(1),
                Vec2::new(50.0, 50.0),
                Layer::new(1),
                1_000,
            )
            .is_none()
        );
    }

    #[test]
    fn find_path_through_layers_and_obstacles() {
        let mut grid = make_grid();
        let layer1 = Layer::new(1);
        let layer2 = Layer::new(2);
        let layer3 = Layer::new(3);

        // Препятствие по X: вертикальная стена.
        grid.add_obstacle(rect(300.0, 0.0, 400.0, 1000.0), &layer1);

        // Препятствие по Y: горизонтальная стена.
        grid.add_obstacle(rect(0.0, 500.0, 1000.0, 600.0), &layer2);

        // Блокируем одну клетку на слое 3.
        grid.add_obstacle(rect(600.0, 600.0, 700.0, 700.0), &layer3);

        // Старт и цель требуют обход по XY и смену слоя.
        let start = Vec2::new(100.0, 100.0);
        let target = Vec2::new(900.0, 900.0);
        let path = grid.find_path(start, layer1, target, layer3, 50_000);

        assert!(path.is_some(), "Path should exist");
        let path = path.expect("path");

        assert!(path.len() > 2, "Path should have multiple segments");

        assert!(
            path_is_valid(&grid, &path),
            "Path must not go through obstacles"
        );

        // Проверяем, что дошли до нужного слоя.
        let last = path.last().expect("last path segment");
        assert_eq!(last.layer.id(), 3);

        // Проверяем, что действительно пришлось менять слой.
        let used_layers = path.iter().map(|p| p.layer.id()).collect::<HashSet<_>>();
        assert!(used_layers.len() > 1, "Path should include layer changes");
    }
}
