use bevy::core::FixedTimestep;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::PresentMode;
use bevy::math::const_vec2;
use bevy::sprite::collide_aabb::{collide, Collision};


// Physics framerate
const TIME_STEP: f32 = 1.0 / 60.0;

const COURT_WIDTH: f32 = 800.0;
const COURT_HEIGHT: f32 = 600.0;

const PADDLE_SIZE: Vec2 = const_vec2!([6., 46.]);
const BALL_SIZE: Vec2 = const_vec2!([8., 8.]);

const BOUNCE_ANGLE_STEEPNESS: f32 = 22.0;


fn main() {
    App::new()
        .insert_resource(WindowDescriptor {
            title: "Bevy Pong".to_string(),
            width: COURT_WIDTH,
            height: COURT_HEIGHT,
            present_mode: PresentMode::Fifo,
            ..default()
        })
        .add_plugins(DefaultPlugins)
        .insert_resource(ClearColor(Color::BLACK))
        .add_startup_system(setup)
        .add_system_set(
            SystemSet::new()
                .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                .with_system(process_collisions_system)
                .with_system(player_controller_system.before(process_collisions_system))
                .with_system(apply_velocity_system.before(process_collisions_system))
            // .with_system(play_collision_sound.after(check_for_collisions))
        )
        .run();
}


#[derive(Component)]
struct Player;


#[derive(Component)]
struct Opponent;


#[derive(Component)]
struct Ball;


#[derive(Component, Debug)]
struct Velocity(Vec2);


#[derive(Component)]
struct Collider;


fn setup(mut windows: ResMut<Windows>, mut commands: Commands) {
    commands.spawn_bundle(OrthographicCameraBundle::new_2d());

    // Grab cursor
    let window = windows.get_primary_mut().unwrap();
    window.set_cursor_lock_mode(true);
    window.set_cursor_visibility(false);

    // Net
    commands.spawn_bundle(SpriteBundle {
        transform: Transform {
            translation: Vec3::ZERO,
            ..default()
        },
        sprite: Sprite {
            color: Color::rgb(0.65, 0.65, 0.65),
            custom_size: Some(Vec2::new(3., COURT_HEIGHT)),
            ..default()
        },
        ..default()
    });

    // Player Paddle (left)
    commands
        .spawn()
        .insert(Player)
        .insert(Collider)
        .insert_bundle(SpriteBundle {
            transform: Transform {
                translation: Vec3::new(-COURT_WIDTH * 0.5 + 26., 0., 0.0),
                ..default()
            },
            sprite: Sprite {
                color: Color::WHITE,
                custom_size: Some(PADDLE_SIZE),
                ..default()
            },
            ..default()
        });

    // Opponent paddle (right)
    commands
        .spawn()
        .insert(Opponent)
        .insert(Collider)
        .insert_bundle(SpriteBundle {
            transform: Transform {
                translation: Vec3::new(COURT_WIDTH * 0.5 - 26., 0., 0.0),
                ..default()
            },
            sprite: Sprite {
                color: Color::WHITE,
                custom_size: Some(PADDLE_SIZE),
                ..default()
            },
            ..default()
        });

    // Ball
    commands
        .spawn()
        .insert(Ball)
        .insert(Velocity(Vec2::new(-300., 0.)))
        .insert_bundle(SpriteBundle {
            transform: Transform {
                translation: Vec3::new(0., 0., 0.0),
                ..default()
            },
            sprite: Sprite {
                color: Color::WHITE,
                custom_size: Some(BALL_SIZE),
                ..default()
            },
            ..default()
        });
}


fn player_controller_system(
    mut query: Query<&mut Transform, With<Player>>,
    mut mouse_motion: EventReader<MouseMotion>,
) {
    let mut player_transform = query.single_mut();

    let accumulated_delta_y: f32 = mouse_motion.iter().map(|motion| {
        // Negate because delta is y-down yet world space is y-up
        -motion.delta.y
    }).sum();

    let new_position = player_transform.translation.y + accumulated_delta_y;

    let lower_bound = -COURT_HEIGHT * 0.5 + (PADDLE_SIZE.y * 0.5) + 5.;
    let upper_bound = COURT_HEIGHT * 0.5 - (PADDLE_SIZE.y * 0.5) - 5.;

    player_transform.translation.y = new_position.clamp(lower_bound, upper_bound);
}


fn apply_velocity_system(mut query: Query<(&mut Transform, &Velocity)>) {
    for (mut transform, velocity) in query.iter_mut() {
        transform.translation.x += velocity.0.x * TIME_STEP;
        transform.translation.y += velocity.0.y * TIME_STEP;
    }
}


fn process_collisions_system(
    mut ball_query: Query<(Entity, &mut Velocity, &Transform, &Sprite), With<Ball>>,
    collider_query: Query<(&Transform, &Sprite), With<Collider>>,
    mut commands: Commands,
) {
    if let Ok((ball, mut ball_velocity, ball_transform, ball_sprite)) = ball_query.get_single_mut() {
        let ball_size = ball_sprite.custom_size.unwrap();

        // Top/bottom walls (bounce)
        let top_wall_collision = collide(
            ball_transform.translation,
            ball_size,
            Vec3::new(0., -COURT_HEIGHT * 0.5 - 20., 0.),
            Vec2::new(COURT_WIDTH, 40.),
        );
        let bottom_wall_collision = collide(
            ball_transform.translation,
            ball_size,
            Vec3::new(0., COURT_HEIGHT * 0.5 + 20., 0.),
            Vec2::new(COURT_WIDTH, 40.),
        );
        if top_wall_collision.is_some() || bottom_wall_collision.is_some() {
            ball_velocity.0.y = -ball_velocity.0.y;
        }

        // Gutters (goal)
        let left_gutter_collision = collide(
            ball_transform.translation,
            ball_size,
            Vec3::new(-COURT_WIDTH * 0.5 + 3., 0., 0.),
            Vec2::new(26., COURT_HEIGHT),
        );
        let right_gutter_collision = collide(
            ball_transform.translation,
            ball_size,
            Vec3::new(COURT_WIDTH * 0.5, 3., 0.),
            Vec2::new(26., COURT_HEIGHT),
        );
        if left_gutter_collision.is_some() {
            println!("Opponent scored!");
            commands.entity(ball).despawn();
        }
        if right_gutter_collision.is_some() {
            println!("Player scored!");
            commands.entity(ball).despawn();
        }

        for (transform, sprite) in collider_query.iter() {
            // Paddle (bounce)
            let collision = collide(
                ball_transform.translation,
                ball_size,
                transform.translation,
                sprite.custom_size.unwrap(),
            );

            let mut bounce_off_paddle = || {
                ball_velocity.0.x = -ball_velocity.0.x;
                let dst_from_center = ball_transform.translation.y - transform.translation.y;
                ball_velocity.0.y = dst_from_center * BOUNCE_ANGLE_STEEPNESS;
            };

            if let Some(collision) = collision {
                match collision {
                    Collision::Left => bounce_off_paddle(),
                    Collision::Right => bounce_off_paddle(),
                    // Ignore other collisions, can only bounce off paddles in X direction
                    _ => (),
                }
            }
        }
    }
}
