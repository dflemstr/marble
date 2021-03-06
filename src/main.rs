extern crate cgmath;
#[macro_use]
extern crate error_chain;
extern crate time;
#[macro_use]
extern crate vulkano;
extern crate vulkano_win;
extern crate winit;

use vulkano_win::VkSurfaceBuild;

use std::sync::Arc;
use std::time::Duration;

mod device;
mod error;
mod teapot;

fn main() {
    let extensions = vulkano_win::required_extensions();
    let instance = vulkano::instance::Instance::new(None, &extensions, None)
        .expect("failed to create instance");

    let window = winit::WindowBuilder::new().build_vk_surface(&instance).unwrap();

    let physical = device::find_best(&instance, &window).unwrap().unwrap();
    println!("Using device: {} (type: {:?})",
             physical.name(),
             physical.ty());

    let queue = physical.queue_families()
        .find(|q| q.supports_graphics() && window.surface().is_supported(q).unwrap_or(false))
        .expect("couldn't find a graphical queue family");

    let device_ext = vulkano::device::DeviceExtensions {
        khr_swapchain: true,
        ..vulkano::device::DeviceExtensions::none()
    };

    let (device, mut queues) = vulkano::device::Device::new(&physical,
                                                            physical.supported_features(),
                                                            &device_ext,
                                                            [(queue, 0.5)].iter().cloned())
        .expect("failed to create device");
    let queue = queues.next().unwrap();

    let (swapchain, images) = {
        let caps = window.surface()
            .get_capabilities(&physical)
            .expect("failed to get surface capabilities");

        let dimensions = caps.current_extent.unwrap_or([1280, 1024]);
        let present = caps.present_modes.iter().next().unwrap();
        let usage = caps.supported_usage_flags;
        let format = caps.supported_formats[0].0;

        vulkano::swapchain::Swapchain::new(&device,
                                           &window.surface(),
                                           3,
                                           format,
                                           dimensions,
                                           1,
                                           &usage,
                                           &queue,
                                           vulkano::swapchain::SurfaceTransform::Identity,
                                           vulkano::swapchain::CompositeAlpha::Opaque,
                                           present,
                                           true,
                                           None)
            .expect("failed to create swapchain")
    };


    let depth_buffer =
        vulkano::image::attachment::AttachmentImage::transient(&device,
                                                               images[0].dimensions(),
                                                               vulkano::format::D16Unorm)
            .unwrap();

    let vertex_buffer = vulkano::buffer::cpu_access::CpuAccessibleBuffer
                                ::from_iter(&device, &vulkano::buffer::BufferUsage::all(), Some(queue.family()), teapot::VERTICES.iter().cloned())
                                .expect("failed to create buffer");

    let normals_buffer = vulkano::buffer::cpu_access::CpuAccessibleBuffer
                                ::from_iter(&device, &vulkano::buffer::BufferUsage::all(), Some(queue.family()), teapot::NORMALS.iter().cloned())
                                .expect("failed to create buffer");

    let index_buffer = vulkano::buffer::cpu_access::CpuAccessibleBuffer
                                ::from_iter(&device, &vulkano::buffer::BufferUsage::all(), Some(queue.family()), teapot::INDICES.iter().cloned())
                                .expect("failed to create buffer");

    let proj = cgmath::perspective(cgmath::Rad(std::f32::consts::FRAC_PI_2),
                                   {
                                       let d = images[0].dimensions();
                                       d[0] as f32 / d[1] as f32
                                   },
                                   0.01,
                                   100.0);
    let view = cgmath::Matrix4::look_at(cgmath::Point3::new(0.3, 0.3, 1.0),
                                        cgmath::Point3::new(0.0, 0.0, 0.0),
                                        cgmath::Vector3::new(0.0, -1.0, 0.0));
    let scale = cgmath::Matrix4::from_scale(0.01);

    let uniform_buffer = vulkano::buffer::cpu_access::CpuAccessibleBuffer::<teapot::vs::ty::Data>
                               ::from_data(&device, &vulkano::buffer::BufferUsage::all(), Some(queue.family()),
                                teapot::vs::ty::Data {
                                    world : <cgmath::Matrix4<f32> as cgmath::SquareMatrix>::identity().into(),
                                    view : (view * scale).into(),
                                    proj : proj.into(),
                                })
                               .expect("failed to create buffer");

    let vs = teapot::vs::Shader::load(&device).expect("failed to create shader module");
    let fs = teapot::fs::Shader::load(&device).expect("failed to create shader module");

    mod renderpass {
        single_pass_renderpass!{
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: ::vulkano::format::Format,
                },
                depth: {
                    load: Clear,
                    store: DontCare,
                    format: ::vulkano::format::D16Unorm,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {depth}
            }
        }
    }

    let renderpass = renderpass::CustomRenderPass::new(&device,
                                                       &renderpass::Formats {
                                                           color: (images[0].format(), 1),
                                                           depth: (vulkano::format::D16Unorm, 1),
                                                       })
        .unwrap();

    let descriptor_pool = vulkano::descriptor::descriptor_set::DescriptorPool::new(&device);

    mod pipeline_layout {
        pipeline_layout!{
            set0: {
                uniforms: UniformBuffer<::teapot::vs::ty::Data>
            }
        }
    }

    let pipeline_layout = pipeline_layout::CustomPipeline::new(&device).unwrap();
    let set = pipeline_layout::set0::Set::new(&descriptor_pool,
                                              &pipeline_layout,
                                              &pipeline_layout::set0::Descriptors {
                                                  uniforms: &uniform_buffer,
                                              });

    let pipeline = vulkano::pipeline::GraphicsPipeline::new(&device, vulkano::pipeline::GraphicsPipelineParams {
        vertex_input: vulkano::pipeline::vertex::TwoBuffersDefinition::new(),
        vertex_shader: vs.main_entry_point(),
        input_assembly: vulkano::pipeline::input_assembly::InputAssembly::triangle_list(),
        tessellation: None,
        geometry_shader: None,
        viewport: vulkano::pipeline::viewport::ViewportsState::Fixed {
            data: vec![(
                vulkano::pipeline::viewport::Viewport {
                    origin: [0.0, 0.0],
                    depth_range: 0.0 .. 1.0,
                    dimensions: [images[0].dimensions()[0] as f32, images[0].dimensions()[1] as f32],
                },
                vulkano::pipeline::viewport::Scissor::irrelevant()
            )],
        },
        raster: Default::default(),
        multisample: vulkano::pipeline::multisample::Multisample::disabled(),
        fragment_shader: fs.main_entry_point(),
        depth_stencil: vulkano::pipeline::depth_stencil::DepthStencil::simple_depth_test(),
        blend: vulkano::pipeline::blend::Blend::pass_through(),
        layout: &pipeline_layout,
        render_pass: vulkano::framebuffer::Subpass::from(&renderpass, 0).unwrap(),
    }).unwrap();

    let framebuffers = images.iter()
        .map(|image| {
            let attachments = renderpass::AList {
                color: &image,
                depth: &depth_buffer,
            };

            vulkano::framebuffer::Framebuffer::new(&renderpass,
                                                   [image.dimensions()[0],
                                                    image.dimensions()[1],
                                                    1],
                                                   attachments)
                .unwrap()
        })
        .collect::<Vec<_>>();


    let command_buffers = framebuffers.iter()
        .map(|framebuffer| {
            vulkano::command_buffer::PrimaryCommandBufferBuilder::new(&device, queue.family())
                .draw_inline(&renderpass,
                             &framebuffer,
                             renderpass::ClearValues {
                                 color: [0.0, 0.0, 1.0, 1.0],
                                 depth: 1.0,
                             })
                .draw_indexed(&pipeline,
                              (&vertex_buffer, &normals_buffer),
                              &index_buffer,
                              &vulkano::command_buffer::DynamicState::none(),
                              &set,
                              &())
                .draw_end()
                .build()
        })
        .collect::<Vec<_>>();

    let mut submissions: Vec<Arc<vulkano::command_buffer::Submission>> = Vec::new();


    loop {
        submissions.retain(|s| s.destroying_would_block());

        {
            let mut buffer_content = uniform_buffer.write(Duration::new(1, 0)).unwrap();

            let rotation =
                cgmath::Matrix3::from_angle_y(cgmath::Rad(time::precise_time_ns() as f32 *
                                                          0.000000001));

            buffer_content.world = cgmath::Matrix4::from(rotation).into();
        }

        let image_num = swapchain.acquire_next_image(Duration::from_millis(1)).unwrap();
        submissions.push(vulkano::command_buffer::submit(&command_buffers[image_num], &queue).unwrap());
        swapchain.present(&queue, image_num).unwrap();

        for ev in window.window().poll_events() {
            match ev {
                winit::Event::Closed => return,
                _ => (),
            }
        }
    }
}
