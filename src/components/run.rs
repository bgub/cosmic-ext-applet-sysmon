use cosmic::{
    cosmic_theme::palette::WithAlpha,
    iced::{core::mouse, Point, Rectangle},
    widget::{
        canvas::{path, stroke, Fill, Frame, Geometry, Program, Stroke},
        Canvas,
    },
    Element, Renderer, Theme,
};

use crate::{applet::Message, color::Color, history::History};

#[derive(Debug)]
pub struct HistoryChart<'a, T = u64> {
    history: &'a History<T>,
    max: T,
    color: Color,
}

impl<'a> HistoryChart<'a> {
    pub fn auto_max(history: &'a History, color: Color) -> HistoryChart<'a> {
        HistoryChart::new(history, *history.iter().max().unwrap_or(&0), color)
    }
}

impl<'a, T> HistoryChart<'a, T> {
    pub fn new(history: &'a History<T>, max: T, color: Color) -> HistoryChart<'a, T> {
        HistoryChart {
            history,
            max,
            color,
        }
    }
}

impl<T: Copy + Ord> HistoryChart<'_, T> {
    pub fn link_max(front: &mut HistoryChart<T>, back: &mut HistoryChart<T>) {
        let max_front = front.max;
        let max_back = back.max;
        let max = max_front.max(max_back);
        front.max = max;
        back.max = max;
    }
}

macro_rules! impl_program_history_chart {
    ($($t:ty),+) => {
        $(
        impl Program<Message, Theme, Renderer> for HistoryChart<'_,$t> {
            type State = ();

            #[allow(clippy::cast_precision_loss)]
            fn draw(
                &self,
                _state: &Self::State,
                renderer: &Renderer,
                theme: &Theme,
                bounds: Rectangle,
                _cursor: mouse::Cursor,
            ) -> Vec<Geometry<Renderer>> {
                let mut fill = Frame::new(renderer, bounds.size());
                let mut line = Frame::new(renderer, bounds.size());
                let color = self.color.as_cosmic_color(theme);

                let x_step = bounds.width / (self.history.len() - 1) as f32;
                let y_step = if self.max as f32 != 0.0 {
                     bounds.height / self.max as f32
                } else {
                    1.0
                };

                // Build the fill path: closed polygon with bottom edge for the
                // filled area under the data line.
                let mut fill_builder = path::Builder::new();
                fill_builder.move_to(Point {
                    x: 0.0,
                    y: bounds.height,
                });
                for (i, j) in self.history.iter().enumerate() {
                    let x = i as f32 * x_step;
                    let y = bounds.height - *j as f32 * y_step;
                    fill_builder.line_to(Point { x, y });
                }
                fill_builder.line_to(Point {
                    x: bounds.width,
                    y: bounds.height,
                });
                let fill_path = fill_builder.build();

                // Build the stroke path: only the data line, no bottom or side
                // edges. Skip zero-value points to avoid drawing a line at the
                // very bottom of the chart when there is no activity.
                let mut line_builder = path::Builder::new();
                let mut need_move = true;
                for (i, j) in self.history.iter().enumerate() {
                    let val = *j as f32;
                    if val == 0.0 {
                        need_move = true;
                        continue;
                    }
                    let x = i as f32 * x_step;
                    let y = bounds.height - val * y_step;
                    let pt = Point { x, y };
                    if need_move {
                        line_builder.move_to(pt);
                        need_move = false;
                    } else {
                        line_builder.line_to(pt);
                    }
                }
                let line_path = line_builder.build();

                fill.fill(
                    &fill_path,
                    Fill {
                        style: stroke::Style::Solid(color.with_alpha(0.5).into()),
                        ..Default::default()
                    },
                );
                line.stroke(
                    &line_path,
                    Stroke {
                        style: stroke::Style::Solid(color.into()),
                        width: 1.0,
                        ..Default::default()
                    },
                );
                vec![fill.into_geometry(),line.into_geometry()]
            }
        })*
    };

}
impl_program_history_chart!(u64, f32);

#[derive(Debug)]
pub struct SimpleHistoryChart<'a, T = u64> {
    history: HistoryChart<'a, T>,
}

macro_rules! impl_program_simple_history_chart {
    ($($t:ty),+) => {
        $(
            impl<'a> From<SimpleHistoryChart<'a, $t>> for Element<'a, Message> {
                fn from(value: SimpleHistoryChart<'a, $t>) -> Self {
                    Canvas::new(value).into()
                }
            }

            impl<'a> Program<Message, Theme, Renderer> for SimpleHistoryChart<'a, $t>{
                type State = ();

                fn draw(
                    &self,
                    state: &Self::State,
                    renderer: &Renderer,
                    theme: &Theme,
                    bounds: Rectangle,
                    cursor: mouse::Cursor,
                ) -> Vec<Geometry<Renderer>> {
                    let mut geometries = Background.draw(state, renderer, theme, bounds, cursor);
                    geometries.extend(self.history.draw(
                        state,
                        renderer,
                        theme,
                        bounds,
                        cursor,
                    ));
                    geometries
                }
            }
        )*
    };
}
impl_program_simple_history_chart!(u64, f32);

impl<'a> SimpleHistoryChart<'a> {
    pub fn auto_max(history: &'a History, color: Color) -> SimpleHistoryChart<'a> {
        SimpleHistoryChart::new(
            history,
            *history.iter().max().unwrap_or(&Default::default()),
            color,
        )
    }
}

impl<'a, T> SimpleHistoryChart<'a, T> {
    pub fn new(history: &'a History<T>, max: T, color: Color) -> SimpleHistoryChart<'a, T> {
        SimpleHistoryChart {
            history: HistoryChart {
                history,
                max,
                color,
            },
        }
    }
}

#[derive(Debug)]
pub struct SuperimposedHistoryChart<'a> {
    pub back: HistoryChart<'a>,
    pub front: HistoryChart<'a>,
}

impl<'a> SuperimposedHistoryChart<'a> {
    pub fn new(
        data_front: &'a History,
        max_front: u64,
        color_front: &Color,
        data_back: &'a History,
        max_back: u64,
        color_back: &Color,
    ) -> Self {
        let back = HistoryChart::new(data_back, max_back, *color_back);
        let front = HistoryChart::new(data_front, max_front, *color_front);
        Self { back, front }
    }

    pub fn new_linked(
        data_front: &'a History<u64>,
        color_front: &Color,
        data_back: &'a History<u64>,
        color_back: &Color,
    ) -> Self {
        let mut back = HistoryChart::auto_max(data_back, *color_back);
        let mut front = HistoryChart::auto_max(data_front, *color_front);
        HistoryChart::link_max(&mut front, &mut back);
        Self { back, front }
    }
}

impl<'a> From<SuperimposedHistoryChart<'a>> for Element<'a, Message> {
    fn from(value: SuperimposedHistoryChart<'a>) -> Self {
        Canvas::new(value).into()
    }
}

impl Program<Message, Theme, Renderer> for SuperimposedHistoryChart<'_> {
    type State = ();

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let mut geometries = Background.draw(state, renderer, theme, bounds, cursor);
        let back = self.back.draw(state, renderer, theme, bounds, cursor);
        let front = self.front.draw(state, renderer, theme, bounds, cursor);
        geometries.extend(back.into_iter().zip(front).flat_map(|(f, b)| [f, b]));
        geometries
    }
}

struct Background;

impl Program<Message, Theme, Renderer> for Background {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let mut frame = Frame::new(renderer, bounds.size());
        let bg_color = theme.cosmic().background.base;

        let mut bg_builder = path::Builder::new();
        let external_bounds = bounds.expand(10.0);
        let Point { x, y } = external_bounds.position();
        bg_builder.move_to(Point { x, y });
        bg_builder.line_to(Point {
            x: x + external_bounds.width,
            y,
        });

        let background = bg_builder.build();

        frame.fill(
            &background,
            Fill {
                style: stroke::Style::Solid(bg_color.into()),
                ..Default::default()
            },
        );
        vec![frame.into_geometry()]
    }
}
