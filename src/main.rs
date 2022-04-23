use bevy::core::FixedTimestep;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::PresentMode;
use bevy::math::const_vec2;
use bevy::sprite::collide_aabb::{collide, Collision};


// Physics framerate
const TIME_STEP: f32 = 1.0 / 60.0;

const WINDOW_WIDTH: f32 = 800.0;
const WINDOW_HEIGHT: f32 = 600.0;

const PADDLE_SIZE: Vec2 = const_vec2!([6., 46.]);
const BALL_SIZE: Vec2 = const_vec2!([8., 8.]);

const BOUNCE_ANGLE_MULTIPLIER: f32 = 22.0;
const BALL_SPEED: f32 = 500.;


fn main() {
    App::new()
        .insert_resource(WindowDescriptor {
            title: "Bevy Pong".to_string(),
            width: WINDOW_WIDTH,
            height: WINDOW_HEIGHT,
            present_mode: PresentMode::Fifo,  // VSync
            ..default()
        })
        .add_plugins(DefaultPlugins)
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(PlayerTurn(true))
        .insert_resource(Scoreboard { player: 0, opponent: 0 })
        .insert_resource(BallSpawnTimer(Timer::from_seconds(0.5, false)))
        .add_event::<CollisionEvent>()
        .add_startup_system(setup)
        .add_system(ball_spawner)
        .add_system(update_scoreboard)
        .add_system_set(
                // Run physics systems (and anything that depends on physics systems) at constant FPS
            SystemSet::new()
                .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                .with_system(player_controller.before(apply_velocity))
                .with_system(opponent_controller.before(apply_velocity))
                .with_system(apply_velocity)
                .with_system(
                    process_collisions
                        .after(player_controller)
                        .after(opponent_controller)
                        .after(apply_velocity)
                )
                .with_system(play_sounds.after(process_collisions))
        )
        .run();
}


// Flag to determine which direction ball starts in
struct PlayerTurn(bool);


// Timer to determine time between ball spawns
struct BallSpawnTimer(Timer);


struct Scoreboard {
    player: u16,
    opponent: u16,
}


// Marker component for player
#[derive(Component)]
struct Player;


// Marker component for opponent
#[derive(Component)]
struct Opponent;


// Marker component for ball
#[derive(Component)]
struct Ball;


// Track velocity of an entity
#[derive(Component)]
struct Velocity(Vec2);


// Marker component for collider
// (collisions based on sprite custom_size)
#[derive(Component)]
struct Collider;


// Marker component for scoreboard text
#[derive(Component)]
struct ScoreText;


enum CollisionEvent {
    Bounce,
    Goal,
}


struct HitSound(Handle<AudioSource>);


struct GoalSound(Handle<AudioSource>);


fn setup(
    mut windows: ResMut<Windows>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
) {
    // Camera
    commands.spawn_bundle(OrthographicCameraBundle::new_2d());

    // Play music and load other sounds
    audio.play_with_settings(
        asset_server.load("sounds/Music.wav"),
        PlaybackSettings::LOOP.with_volume(0.1),
    );
    let hit_sound = asset_server.load("sounds/PaddleHitSound.wav");
    let goal_sound = asset_server.load("sounds/GoalSound.wav");
    commands.insert_resource(HitSound(hit_sound));
    commands.insert_resource(GoalSound(goal_sound));

    // Grab and hide cursor
    let window = windows.get_primary_mut().unwrap();
    window.set_cursor_lock_mode(true);
    window.set_cursor_visibility(false);

    // Draw net (line in middle)
    commands.spawn_bundle(SpriteBundle {
        transform: Transform {
            translation: Vec3::ZERO,
            ..default()
        },
        sprite: Sprite {
            color: Color::rgb(0.65, 0.65, 0.65),
            custom_size: Some(Vec2::new(3., WINDOW_HEIGHT)),
            ..default()
        },
        ..default()
    });

    // Add player Paddle (left)
    commands
        .spawn()
        .insert(Player)
        .insert(Collider)
        .insert_bundle(SpriteBundle {
            transform: Transform {
                translation: Vec3::new(-WINDOW_WIDTH * 0.5 + 26., 0., 0.0),
                ..default()
            },
            sprite: Sprite {
                color: Color::WHITE,
                custom_size: Some(PADDLE_SIZE),
                ..default()
            },
            ..default()
        });

    // Add opponent paddle (right)
    commands
        .spawn()
        .insert(Opponent)
        .insert(Collider)
        .insert(Velocity(Vec2::ZERO))
        .insert_bundle(SpriteBundle {
            transform: Transform {
                translation: Vec3::new(WINDOW_WIDTH * 0.5 - 26., 0., 0.0),
                ..default()
            },
            sprite: Sprite {
                color: Color::WHITE,
                custom_size: Some(PADDLE_SIZE),
                ..default()
            },
            ..default()
        });

    // UI Camera
    commands.spawn_bundle(UiCameraBundle::default());

    // Scoreboard
    commands
        .spawn_bundle(NodeBundle {
            style: Style {
                size: Size::new(Val::Percent(100.), Val::Percent(100.)),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::FlexEnd,  // Coordinates are Y-up so this is at top of screen
                ..default()
            },
            color: Color::NONE.into(),
            ..default()
        })
        .with_children(|parent| {
            parent.spawn_bundle(TextBundle {
                style: Style {
                    margin: Rect {
                        top: Val::Percent(7.),
                        ..default()
                    },
                    ..default()
                },
                text: Text {
                    sections: vec![
                        TextSection {
                            value: "0".to_string(),
                            style: TextStyle {
                                font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                                font_size: 60.0,
                                color: Color::WHITE,
                            },
                        },
                        // Spacer hack so I can update both scores with a single entity/component
                        TextSection {
                            value: "               ".to_string(),
                            style: TextStyle {
                                font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                                font_size: 60.0,
                                color: Color::WHITE,
                            },
                        },
                        TextSection {
                            value: "0".to_string(),
                            style: TextStyle {
                                font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                                font_size: 60.0,
                                color: Color::WHITE,
                            },
                        },
                    ],
                    ..default()
                },
                ..default()
            })
                .insert(ScoreText);
        });
}


/// Controls the player paddle with the mouse
fn player_controller(
    mut query: Query<&mut Transform, With<Player>>,
    mut mouse_motion: EventReader<MouseMotion>,
) {
    let mut player_transform = query.single_mut();

    let accumulated_delta_y: f32 = mouse_motion.iter().map(|motion| {
        // Negate because delta is y-down yet world space is y-up
        -motion.delta.y
    }).sum();

    let new_position = player_transform.translation.y + accumulated_delta_y;

    // Prevent paddle going off-screen
    let lower_bound = -WINDOW_HEIGHT * 0.5 + (PADDLE_SIZE.y * 0.5) + 5.;
    let upper_bound = WINDOW_HEIGHT * 0.5 - (PADDLE_SIZE.y * 0.5) - 5.;

    player_transform.translation.y = new_position.clamp(lower_bound, upper_bound);
}


/// Generic system to apply velocity to any entity with velocity and transform components
fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>) {
    for (mut transform, velocity) in query.iter_mut() {
        transform.translation.x += velocity.0.x * TIME_STEP;
        transform.translation.y += velocity.0.y * TIME_STEP;
    }
}


/// Detect ball collisions and act accordingly
///  - Bounce off walls and paddles
///  - Increment scores if hit goals
///  - Play sounds
fn process_collisions(
    mut ball_query: Query<(Entity, &mut Velocity, &Transform, &Sprite), With<Ball>>,
    collider_query: Query<(&Transform, &Sprite), With<Collider>>,
    mut ball_spawn_timer: ResMut<BallSpawnTimer>,
    mut scoreboard: ResMut<Scoreboard>,
    mut collision_events: EventWriter<CollisionEvent>,
    mut commands: Commands,
) {
    if let Ok((ball, mut ball_velocity, ball_transform, ball_sprite)) = ball_query.get_single_mut() {
        let ball_size = ball_sprite.custom_size.unwrap();

        // Top/bottom walls (bounce)
        let top_wall_collision = collide(
            ball_transform.translation,
            ball_size,
            Vec3::new(0., -WINDOW_HEIGHT * 0.5 - 20., 0.),
            Vec2::new(WINDOW_WIDTH, 40.),
        );
        let bottom_wall_collision = collide(
            ball_transform.translation,
            ball_size,
            Vec3::new(0., WINDOW_HEIGHT * 0.5 + 20., 0.),
            Vec2::new(WINDOW_WIDTH, 40.),
        );
        if top_wall_collision.is_some() || bottom_wall_collision.is_some() {
            ball_velocity.0.y = -ball_velocity.0.y;
            collision_events.send(CollisionEvent::Bounce);
        }

        // Gutters (goal)
        let left_gutter_collision = collide(
            ball_transform.translation,
            ball_size,
            Vec3::new(-WINDOW_WIDTH * 0.5 + 3., 0., 0.),
            Vec2::new(26., WINDOW_HEIGHT),
        );
        let right_gutter_collision = collide(
            ball_transform.translation,
            ball_size,
            Vec3::new(WINDOW_WIDTH * 0.5, 3., 0.),
            Vec2::new(26., WINDOW_HEIGHT),
        );
        if left_gutter_collision.is_some() {
            commands.entity(ball).despawn();
            ball_spawn_timer.0.reset();
            scoreboard.opponent += 1;
            collision_events.send(CollisionEvent::Goal);
        }
        if right_gutter_collision.is_some() {
            commands.entity(ball).despawn();
            ball_spawn_timer.0.reset();
            scoreboard.player += 1;
            collision_events.send(CollisionEvent::Goal);
        }

        // Iterate over other colliders (only paddles)
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
                // Determine Y-velocity based on where on the paddle it hit
                let dst_from_center = ball_transform.translation.y - transform.translation.y;
                ball_velocity.0.y = dst_from_center * BOUNCE_ANGLE_MULTIPLIER;
                collision_events.send(CollisionEvent::Bounce);
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


/// Spawn the ball, alternating direction, based on fixed spawn timer
fn ball_spawner(
    mut commands: Commands,
    time: Res<Time>,
    mut ball_spawn_timer: ResMut<BallSpawnTimer>,
    mut player_turn: ResMut<PlayerTurn>,
) {
    if ball_spawn_timer.0.tick(time.delta()).just_finished() {
        // Determine which direction ball starts
        let dir_multiplier = if player_turn.0 { -1.0 } else { 1.0 };

        // Spawn ball
        commands
            .spawn()
            .insert(Ball)
            .insert(Velocity(Vec2::new(BALL_SPEED * dir_multiplier, 0.)))
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

        // Switch turns
        player_turn.0 = !player_turn.0;
    }
}


/// Very basic AI for opponent
///  - If ball does not exist or is moving away from opponent, then stop
///  - If ball is moving toward opponent, then set Y-velocity based on distance to ball on Y-axis
fn opponent_controller(
    ball_query: Query<(&Transform, &Velocity), With<Ball>>,
    mut opponent_query: Query<(&Opponent, &Transform, &mut Velocity), Without<Ball>>,
) {
    let (_, opponent_transform, mut opponent_velocity) = opponent_query.single_mut();

    if let Ok((ball_transform, ball_velocity)) = ball_query.get_single() {
        if ball_velocity.0.x > 0.0 {
            opponent_velocity.0.y = (
                (ball_transform.translation.y - opponent_transform.translation.y) * 13.
            ).clamp(-450., 450.);
        } else {
            opponent_velocity.0.y = 0.;
        }
    } else {
        opponent_velocity.0.y = 0.;
    }
}


/// Update scoreboard text based on current score
fn update_scoreboard(
    scoreboard: Res<Scoreboard>,
    mut score_query: Query<&mut Text, With<ScoreText>>,
) {
    let mut score_text = score_query.single_mut();

    score_text.sections[0].value = format!("{}", scoreboard.player);
    score_text.sections[2].value = format!("{}", scoreboard.opponent);
}


/// Play appropriate collision sounds in response to collision events
fn play_sounds(
    mut collision_events: EventReader<CollisionEvent>,
    audio: Res<Audio>,
    hit_sound: Res<HitSound>,
    goal_sound: Res<GoalSound>,
) {
    for event in collision_events.iter() {
        match event {
            CollisionEvent::Bounce => audio.play(hit_sound.0.clone()),
            CollisionEvent::Goal => {
                audio.play_with_settings(
                    goal_sound.0.clone(),
                    PlaybackSettings::ONCE.with_volume(0.4)
                )
            },
        };
    }
}
