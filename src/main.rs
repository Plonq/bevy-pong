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
        .insert_resource(PlayerTurn(true))
        .insert_resource(Scoreboard { player: 0, opponent: 0 })
        .insert_resource(BallSpawnTimer(Timer::from_seconds(0.5, false)))
        .add_event::<CollisionEvent>()
        .add_startup_system(setup)
        .add_system(ball_spawn_system)
        .add_system(scoreboard_update_system)
        .add_system_set(
            SystemSet::new()
                .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                .with_system(player_controller_system.before(apply_velocity_system))
                .with_system(opponent_controller_system.before(apply_velocity_system))
                .with_system(apply_velocity_system)
                .with_system(
                    process_collisions_system
                        .after(player_controller_system)
                        .after(opponent_controller_system)
                        .after(apply_velocity_system)
                )
                .with_system(collision_sound_system.after(process_collisions_system))
        )
        .run();
}


struct PlayerTurn(bool);


struct BallSpawnTimer(Timer);


struct Scoreboard {
    player: u16,
    opponent: u16,
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


#[derive(Component)]
struct ScoreText;


enum CollisionEvent {
    Bounce,
    Goal,
}


struct CollisionSound(Handle<AudioSource>);


struct GoalSound(Handle<AudioSource>);


fn setup(
    mut windows: ResMut<Windows>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
) {
    commands.spawn_bundle(OrthographicCameraBundle::new_2d());

    // Sounds
    audio.play_with_settings(
        asset_server.load("sounds/Music.wav"),
        PlaybackSettings::LOOP.with_volume(0.1),
    );
    let hit_sound = asset_server.load("sounds/PaddleHitSound.wav");
    let goal_sound = asset_server.load("sounds/GoalSound.wav");
    commands.insert_resource(CollisionSound(hit_sound));
    commands.insert_resource(GoalSound(goal_sound));

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
        .insert(Velocity(Vec2::ZERO))
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

    // UI
    commands.spawn_bundle(UiCameraBundle::default());
    // Player Score
    commands
        .spawn_bundle(NodeBundle {
            style: Style {
                size: Size::new(Val::Percent(100.), Val::Percent(100.)),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::FlexEnd,
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
                        // Spacer
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
            collision_events.send(CollisionEvent::Bounce);
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


fn ball_spawn_system(
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
            .insert(Velocity(Vec2::new(300. * dir_multiplier, 0.)))
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


fn opponent_controller_system(
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


fn scoreboard_update_system(
    scoreboard: Res<Scoreboard>,
    mut score_query: Query<&mut Text, With<ScoreText>>,
) {
    let mut score_text = score_query.single_mut();

    score_text.sections[0].value = format!("{}", scoreboard.player);
    score_text.sections[2].value = format!("{}", scoreboard.opponent);
}


fn collision_sound_system(
    mut collision_events: EventReader<CollisionEvent>,
    audio: Res<Audio>,
    collision_sound: Res<CollisionSound>,
    goal_sound: Res<GoalSound>,
) {
    for event in collision_events.iter() {
        match event {
            CollisionEvent::Bounce => audio.play(collision_sound.0.clone()),
            CollisionEvent::Goal => {
                audio.play_with_settings(
                    goal_sound.0.clone(),
                    PlaybackSettings::ONCE.with_volume(0.4)
                )
            },
        };
    }
}
