// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    app::{Core, Task},
    cosmic_config::{self, ConfigSet},
    iced::{window::Id, Rectangle, Subscription},
    surface::{
        self,
        action::{app_popup, destroy_popup},
    },
    widget::{self, container, list_column, settings::item::builder as settings_builder},
    Application, Element, Theme,
};
use std::time::Duration;
use sysinfo::{
    CpuRefreshKind, Disk, DiskRefreshKind, Disks, MemoryRefreshKind, Networks, RefreshKind, System,
};

use crate::{
    components::gpu::Gpus,
    config::{config_subscription, ComponentConfig, ComponentKind, Config},
    history::History,
    views::format_bytes,
};

pub const ID: &str = "com.github.bgub.CosmicExtAppletSysmon";

pub struct SystemMonitorApplet {
    pub core: Core,
    pub config: Config,
    #[allow(dead_code)]
    config_handler: Option<cosmic_config::Config>,
    popup: Option<Id>,

    pub sys: System,
    pub nets: Networks,
    pub disks: Disks,
    pub gpus: Gpus,
    /// percentage global cpu used between refreshes
    pub global_cpu: History<f32>,
    pub ram: History,
    pub swap: History,
    /// amount uploaded between refresh of `sysinfo::Nets`. (DOES NOT STORE RATE)
    pub upload: History,
    /// amount downloaded between refresh of `sysinfo::Nets`. (DOES NOT STORE RATE)
    pub download: History,
    /// amount read between refresh of `sysinfo::Disks`. (DOES NOT STORE RATE)
    pub disk_read: History,
    /// amount written between refresh of `sysinfo::Disks`. (DOES NOT STORE RATE)
    pub disk_write: History,
    /// amount read between refresh of `sysinfo::Disks`. (DOES NOT STORE RATE)
    pub gpu_usage: Vec<History>,
    /// amount written between refresh of `sysinfo::Disks`. (DOES NOT STORE RATE)
    pub vram: Vec<History>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Config(Config),
    TickCpu,
    TickMem,
    TickNet,
    TickDisk,
    TickGpu,
    ToggleVisibility(ComponentKind),
    PopupClosed(Id),
    Surface(surface::Action),
}

#[derive(Clone, Debug)]
pub struct Flags {
    pub config_handler: Option<cosmic_config::Config>,
    pub config: Config,
}

impl Application for SystemMonitorApplet {
    type Executor = cosmic::executor::Default;

    type Flags = Flags;

    type Message = Message;

    const APP_ID: &'static str = ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let (mut cpu, mut mem, mut net, mut disk, mut gpu) = Default::default();
        let sampling = &flags.config.sampling;
        for chart_config in &flags.config.components {
            match chart_config {
                ComponentConfig::Cpu(_) => cpu = sampling.cpu.sampling_window,
                ComponentConfig::Mem(_) => mem = sampling.mem.sampling_window,
                ComponentConfig::Net(_) => net = sampling.net.sampling_window,
                ComponentConfig::Disk(_) => disk = sampling.disk.sampling_window,
                ComponentConfig::Gpu(_) => gpu = sampling.gpu.sampling_window,
            }
        }
        let gpus = Gpus::new();
        let app = Self {
            core,
            config: flags.config,
            config_handler: flags.config_handler,
            popup: None,

            global_cpu: History::with_capacity(cpu),
            ram: History::with_capacity(mem),
            swap: History::with_capacity(mem),
            upload: History::with_capacity(net),
            download: History::with_capacity(net),
            disk_read: History::with_capacity(disk),
            disk_write: History::with_capacity(disk),
            gpu_usage: vec![History::with_capacity(gpu); gpus.num_gpus()],
            vram: vec![History::with_capacity(gpu); gpus.num_gpus()],

            sys: System::new_with_specifics(
                RefreshKind::nothing()
                    .with_cpu(CpuRefreshKind::nothing().with_cpu_usage())
                    .with_memory(MemoryRefreshKind::everything()),
            ),
            nets: Networks::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list_specifics(
                DiskRefreshKind::nothing().with_io_usage(),
            ),
            gpus,
        };

        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn view(&'_ self) -> Element<'_, Message> {
        let visibility = &self.config.visibility;

        let content: Element<'_, Message> = if visibility.any_visible() {
            let item_iter = self.config.components.iter().filter_map(|module| {
                let (kind, elements) = match module {
                    ComponentConfig::Cpu(vis) => (ComponentKind::Cpu, self.cpu_view(vis)),
                    ComponentConfig::Mem(vis) => (ComponentKind::Mem, self.mem_view(vis)),
                    ComponentConfig::Net(vis) => (ComponentKind::Net, self.net_view(vis)),
                    ComponentConfig::Disk(vis) => (ComponentKind::Disk, self.disk_view(vis)),
                    ComponentConfig::Gpu(vis) => (ComponentKind::Gpu, self.gpu_view(vis)),
                };
                if !visibility.get(kind) || elements.is_empty() {
                    return None;
                }
                Some(self.panel_collection(elements, self.config.layout.inner_spacing, 0.0))
            });

            let items =
                self.panel_collection(item_iter, self.config.layout.spacing, self.padding());
            self.core.applet.autosize_window(items).into()
        } else {
            widget::icon::from_name(ID).size(24).icon().into()
        };

        let have_popup = self.popup;
        widget::button::custom(content)
            .class(cosmic::theme::Button::AppletIcon)
            .on_press_with_rectangle(move |offset, bounds| {
                if let Some(id) = have_popup {
                    Message::Surface(destroy_popup(id))
                } else {
                    #[allow(clippy::cast_possible_truncation)]
                    Message::Surface(app_popup::<SystemMonitorApplet>(
                        move |state: &mut SystemMonitorApplet| {
                            let new_id = Id::unique();
                            state.popup = Some(new_id);
                            let mut popup_settings = state.core.applet.get_popup_settings(
                                state.core.main_window_id().unwrap(),
                                new_id,
                                None,
                                None,
                                None,
                            );
                            popup_settings.positioner.anchor_rect = Rectangle {
                                x: (bounds.x - offset.x) as i32,
                                y: (bounds.y - offset.y) as i32,
                                width: bounds.width as i32,
                                height: bounds.height as i32,
                            };
                            popup_settings
                        },
                        Some(Box::new(|state: &SystemMonitorApplet| {
                            Element::from(state.core.applet.popup_container(state.popup_view()))
                                .map(cosmic::Action::App)
                        })),
                    ))
                }
            })
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        // Fallback — popup views are registered via app_popup closures
        widget::text("").into()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }
            Message::ToggleVisibility(kind) => {
                self.config.visibility.toggle(kind);
                if let Some(handler) = &self.config_handler {
                    let tx = handler.transaction();
                    let _ = ConfigSet::set(&tx, "visibility", self.config.visibility);
                    let _ = tx.commit();
                }
            }
            Message::Config(config) => {
                self.config = config;
                let sampĺing = &self.config.sampling;
                self.global_cpu.resize(sampĺing.cpu.sampling_window);
                self.ram.resize(sampĺing.mem.sampling_window);
                self.swap.resize(sampĺing.mem.sampling_window);
                self.upload.resize(sampĺing.net.sampling_window);
                self.download.resize(sampĺing.net.sampling_window);
                self.disk_read.resize(sampĺing.disk.sampling_window);
                self.disk_write.resize(sampĺing.disk.sampling_window);
                for i in 0..self.gpus.num_gpus() {
                    self.gpu_usage[i].resize(sampĺing.cpu.sampling_window);
                    self.vram[i].resize(sampĺing.cpu.sampling_window);
                }
            }
            Message::TickCpu => {
                self.sys.refresh_cpu_usage();
                self.global_cpu.push(self.sys.global_cpu_usage());
            }
            Message::Surface(a) => {
                return cosmic::task::message(cosmic::Action::Cosmic(
                    cosmic::app::Action::Surface(a),
                ));
            }
            Message::TickMem => {
                self.sys.refresh_memory();
                self.ram.push(self.sys.used_memory());
                self.swap.push(self.sys.used_swap());
            }
            Message::TickNet => {
                self.nets.refresh(true);
                let (received, transmitted) =
                    self.nets.iter().fold((0, 0), |(acc_r, acc_t), (_, data)| {
                        (acc_r + data.received(), acc_t + data.transmitted())
                    });
                self.upload.push(transmitted);
                self.download.push(received);
            }
            Message::TickDisk => {
                self.disks
                    .refresh_specifics(true, DiskRefreshKind::nothing().with_io_usage());
                let (read, written) = self
                    .disks
                    .iter()
                    .map(Disk::usage)
                    .fold((0, 0), |(acc_r, acc_w), usage| {
                        (acc_r + usage.read_bytes, acc_w + usage.written_bytes)
                    });
                self.disk_read.push(read);
                self.disk_write.push(written);
            }
            Message::TickGpu => {
                self.gpus.refresh();
                for (idx, data) in self.gpus.data().iter().enumerate() {
                    self.gpu_usage[idx].push(data.usage);
                    self.vram[idx].push(data.used_vram);
                }
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let mut subs = Vec::new();
        let sampling = &self.config.sampling;
        for chart in &self.config.components {
            let tick = {
                match chart {
                    ComponentConfig::Cpu(_) => cosmic::iced::time::every(Duration::from_millis(
                        sampling.cpu.update_interval,
                    ))
                    .map(|_| Message::TickCpu),
                    ComponentConfig::Mem(_) => cosmic::iced::time::every(Duration::from_millis(
                        sampling.mem.update_interval,
                    ))
                    .map(|_| Message::TickMem),
                    ComponentConfig::Net(_) => cosmic::iced::time::every(Duration::from_millis(
                        sampling.net.update_interval,
                    ))
                    .map(|_| Message::TickNet),
                    ComponentConfig::Disk(_) => cosmic::iced::time::every(Duration::from_millis(
                        sampling.disk.update_interval,
                    ))
                    .map(|_| Message::TickDisk),
                    ComponentConfig::Gpu(_) => cosmic::iced::time::every(Duration::from_millis(
                        sampling.gpu.update_interval,
                    ))
                    .map(|_| Message::TickGpu),
                }
            };
            subs.push(tick);
        }

        subs.push(config_subscription());

        Subscription::batch(subs)
    }

    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(cosmic::applet::style())
    }
}

impl SystemMonitorApplet {
    fn popup_view(&self) -> Element<'_, Message> {
        let vis = &self.config.visibility;
        let mut items = list_column().padding(5).spacing(0);

        // CPU
        items = items.add(
            settings_builder(format!("CPU: {:.1}%", self.sys.global_cpu_usage()))
                .toggler(vis.cpu, |_| Message::ToggleVisibility(ComponentKind::Cpu)),
        );

        // Memory (RAM + Swap nested)
        let total_swap = self.sys.total_swap();
        let mem_description = if total_swap > 0 {
            format!(
                "RAM: {} / {}  |  Swap: {} / {}",
                format_bytes(self.sys.used_memory()),
                format_bytes(self.sys.total_memory()),
                format_bytes(self.sys.used_swap()),
                format_bytes(total_swap),
            )
        } else {
            format!(
                "RAM: {} / {}",
                format_bytes(self.sys.used_memory()),
                format_bytes(self.sys.total_memory()),
            )
        };
        items = items.add(
            settings_builder("Memory")
                .description(mem_description)
                .toggler(vis.mem, |_| Message::ToggleVisibility(ComponentKind::Mem)),
        );

        // Network
        let upload = self.upload.iter().last().copied().unwrap_or(0);
        let download = self.download.iter().last().copied().unwrap_or(0);
        items = items.add(
            settings_builder("Network")
                .description(format!(
                    "Down: {}/s  |  Up: {}/s",
                    format_bytes(download),
                    format_bytes(upload),
                ))
                .toggler(vis.net, |_| Message::ToggleVisibility(ComponentKind::Net)),
        );

        // Disk
        let disk_read = self.disk_read.iter().last().copied().unwrap_or(0);
        let disk_write = self.disk_write.iter().last().copied().unwrap_or(0);
        items = items.add(
            settings_builder("Disk")
                .description(format!(
                    "Read: {}/s  |  Write: {}/s",
                    format_bytes(disk_read),
                    format_bytes(disk_write),
                ))
                .toggler(vis.disk, |_| {
                    Message::ToggleVisibility(ComponentKind::Disk)
                }),
        );

        // GPU
        let gpu_data = self.gpus.data();
        if !gpu_data.is_empty() {
            let gpu_desc = gpu_data
                .iter()
                .enumerate()
                .map(|(idx, data)| {
                    format!(
                        "GPU{idx}: {}%  VRAM: {} / {}",
                        data.usage,
                        format_bytes(data.used_vram),
                        format_bytes(data.total_vram),
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            items = items.add(
                settings_builder("GPU")
                    .description(gpu_desc)
                    .toggler(vis.gpu, |_| {
                        Message::ToggleVisibility(ComponentKind::Gpu)
                    }),
            );
        }

        items.into()
    }
}

pub fn base_background(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(cosmic::iced::Color::from(theme.cosmic().primary.base).into()),
        ..container::Style::default()
    }
}
