use std::env;
use std::ffi::c_void;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::raw::{c_char, c_int, c_short, c_uchar};
use std::path::{Path, PathBuf};

const SILK_HEADER: &str = "#!SILK_V3";
const TENCENT_PREFIX: u8 = 0x02;
const MAX_BYTES_PER_FRAME: usize = 250;
const MAX_INPUT_FRAMES: usize = 5;
const FRAME_LENGTH_MS: i32 = 20;
const MAX_FRAME_LENGTH_MS: i32 = 60;
const MAX_API_FS_HZ: i32 = 48_000;
const MAX_INTERNAL_FS_HZ: i32 = 24_000;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct SKP_SILK_SDK_EncControlStruct {
    api_sample_rate: c_int,
    max_internal_sample_rate: c_int,
    packet_size: c_int,
    bit_rate: c_int,
    packet_loss_percentage: c_int,
    complexity: c_int,
    use_inband_fec: c_int,
    use_dtx: c_int,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct SKP_SILK_SDK_DecControlStruct {
    api_sample_rate: c_int,
    frame_size: c_int,
    frames_per_packet: c_int,
    more_internal_decoder_frames: c_int,
    inband_fec_offset: c_int,
}

unsafe extern "C" {
    fn SKP_Silk_SDK_Get_Encoder_Size(enc_size_bytes: *mut c_int) -> c_int;
    fn SKP_Silk_SDK_InitEncoder(
        enc_state: *mut c_void,
        enc_status: *mut SKP_SILK_SDK_EncControlStruct,
    ) -> c_int;
    fn SKP_Silk_SDK_Encode(
        enc_state: *mut c_void,
        enc_control: *const SKP_SILK_SDK_EncControlStruct,
        samples_in: *const c_short,
        n_samples_in: c_int,
        out_data: *mut c_uchar,
        n_bytes_out: *mut c_short,
    ) -> c_int;

    fn SKP_Silk_SDK_Get_Decoder_Size(dec_size_bytes: *mut c_int) -> c_int;
    fn SKP_Silk_SDK_InitDecoder(dec_state: *mut c_void) -> c_int;
    fn SKP_Silk_SDK_Decode(
        dec_state: *mut c_void,
        dec_control: *mut SKP_SILK_SDK_DecControlStruct,
        lost_flag: c_int,
        in_data: *const c_uchar,
        n_bytes_in: c_int,
        samples_out: *mut c_short,
        n_samples_out: *mut c_short,
    ) -> c_int;

    fn SKP_Silk_SDK_get_version() -> *const c_char;
}

#[derive(Debug)]
struct EncodeArgs {
    input: PathBuf,
    output: PathBuf,
    sample_rate: i32,
    sample_rate_set: bool,
    max_internal_rate: i32,
    max_internal_set: bool,
    packet_ms: i32,
    bitrate: i32,
    loss: i32,
    fec: i32,
    dtx: i32,
    complexity: i32,
    tencent: bool,
    stats: bool,
    quiet: bool,
}

#[derive(Debug)]
struct DecodeArgs {
    input: PathBuf,
    output: PathBuf,
    sample_rate: i32,
    wav: bool,
    tolerant: TolerantMode,
    stats: bool,
    reference: Option<PathBuf>,
    quiet: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TolerantMode {
    Off,
    Skip,
    Silence,
}

fn print_usage() {
    eprintln!(
        "rust-silk <encode|decode> [options]\n\n\
encode options:\n\
  -i, --input <path>           input PCM s16le or WAV (or - for stdin)\n\
  -o, --output <path>          output .silk (or - for stdout)\n\
  --sample-rate <Hz>           default 24000\n\
  --max-internal <Hz>          default = min(sample-rate, 24000)\n\
  --packet-ms <ms>             default 20\n\
  --bitrate <bps>              default 25000\n\
  --loss <percent>             default 0\n\
  --fec <0|1>                   default 0\n\
  --dtx <0|1>                   default 0\n\
  --complexity <0|1|2>          default 2\n\
  --tencent                     write 0x02 prefix + SILK header\n\
  --stats                       print encoding stats\n\
  --quiet                       suppress progress\n\n\
decode options:\n\
  -i, --input <path>           input .silk (or - for stdin)\n\
  -o, --output <path>          output PCM s16le (or .wav, or - for stdout)\n\
  --sample-rate <Hz>           default 24000\n\
  --wav                         write WAV header\n\
  --tolerant <skip|silence>     handle broken packets\n\
  --metrics                     print decode metrics\n\
  --reference <path>            compare against PCM/WAV reference\n\
  --quiet                       suppress progress\n"
    );
}

fn parse_encode_args(mut args: impl Iterator<Item = String>) -> Result<EncodeArgs, String> {
    let mut input = None;
    let mut output = None;
    let mut sample_rate = 24_000;
    let mut sample_rate_set = false;
    let mut max_internal_rate = 24_000;
    let mut max_internal_set = false;
    let mut packet_ms = 20;
    let mut bitrate = 25_000;
    let mut loss = 0;
    let mut fec = 0;
    let mut dtx = 0;
    let mut complexity = 2;
    let mut tencent = false;
    let mut stats = false;
    let mut quiet = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-i" | "--input" => {
                input = args.next().map(PathBuf::from);
            }
            "-o" | "--output" => {
                output = args.next().map(PathBuf::from);
            }
            "--sample-rate" | "--fs" => {
                sample_rate = parse_i32(args.next(), "sample-rate")?;
                sample_rate_set = true;
                if !max_internal_set {
                    max_internal_rate = default_max_internal_rate(sample_rate);
                }
            }
            "--max-internal" => {
                max_internal_rate = parse_i32(args.next(), "max-internal")?;
                max_internal_set = true;
            }
            "--packet-ms" => {
                packet_ms = parse_i32(args.next(), "packet-ms")?;
            }
            "--bitrate" => {
                bitrate = parse_i32(args.next(), "bitrate")?;
            }
            "--loss" => {
                loss = parse_i32(args.next(), "loss")?;
            }
            "--fec" => {
                fec = parse_i32(args.next(), "fec")?;
            }
            "--dtx" => {
                dtx = parse_i32(args.next(), "dtx")?;
            }
            "--complexity" => {
                complexity = parse_i32(args.next(), "complexity")?;
            }
            "--tencent" => tencent = true,
            "--stats" => stats = true,
            "--quiet" => quiet = true,
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            other => return Err(format!("unknown arg: {other}")),
        }
    }

    let input = input.ok_or("missing --input")?;
    let output = output.ok_or("missing --output")?;

    Ok(EncodeArgs {
        input,
        output,
        sample_rate,
        sample_rate_set,
        max_internal_rate,
        max_internal_set,
        packet_ms,
        bitrate,
        loss,
        fec,
        dtx,
        complexity,
        tencent,
        stats,
        quiet,
    })
}

fn parse_decode_args(mut args: impl Iterator<Item = String>) -> Result<DecodeArgs, String> {
    let mut input = None;
    let mut output = None;
    let mut sample_rate = 24_000;
    let mut wav = false;
    let mut tolerant = TolerantMode::Off;
    let mut stats = false;
    let mut reference = None;
    let mut quiet = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-i" | "--input" => {
                input = args.next().map(PathBuf::from);
            }
            "-o" | "--output" => {
                output = args.next().map(PathBuf::from);
            }
            "--sample-rate" | "--fs" => {
                sample_rate = parse_i32(args.next(), "sample-rate")?;
            }
            "--wav" => wav = true,
            "--tolerant" => {
                let mode = args
                    .next()
                    .ok_or_else(|| "missing value for tolerant".to_string())?;
                tolerant = match mode.as_str() {
                    "skip" => TolerantMode::Skip,
                    "silence" => TolerantMode::Silence,
                    other => return Err(format!("invalid tolerant mode: {other}")),
                };
            }
            "--metrics" => stats = true,
            "--reference" | "--ref" => {
                reference = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| "missing value for reference".to_string())?,
                ));
                stats = true;
            }
            "--quiet" => quiet = true,
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            other => return Err(format!("unknown arg: {other}")),
        }
    }

    let input = input.ok_or("missing --input")?;
    let output = output.ok_or("missing --output")?;

    Ok(DecodeArgs {
        input,
        output,
        sample_rate,
        wav,
        tolerant,
        stats,
        reference,
        quiet,
    })
}

fn parse_i32(value: Option<String>, name: &str) -> Result<i32, String> {
    let value = value.ok_or_else(|| format!("missing value for {name}"))?;
    value
        .parse::<i32>()
        .map_err(|_| format!("invalid {name}: {value}"))
}

fn encode(mut args: EncodeArgs) -> Result<(), String> {
    let input_source = open_input(&args.input)?;
    let mut input = input_source.reader;
    if let Some(wav) = input_source.wav {
        if args.sample_rate_set && args.sample_rate != wav.sample_rate {
            return Err(format!(
                "sample-rate ({}) does not match WAV ({})",
                args.sample_rate, wav.sample_rate
            ));
        }
        if !args.sample_rate_set {
            args.sample_rate = wav.sample_rate;
        }
        if !args.max_internal_set {
            args.max_internal_rate = default_max_internal_rate(args.sample_rate);
        }
    }

    if args.sample_rate <= 0 || args.sample_rate > MAX_API_FS_HZ {
        return Err(format!(
            "sample-rate out of range (8000-48000): {}",
            args.sample_rate
        ));
    }
    if !is_supported_sample_rate(args.sample_rate) {
        return Err(format!(
            "sample-rate must be one of 8000, 12000, 16000, 24000, 48000: {}",
            args.sample_rate
        ));
    }
    if args.max_internal_rate <= 0 || args.max_internal_rate > MAX_INTERNAL_FS_HZ {
        return Err(format!(
            "max-internal out of range (8000-24000): {}",
            args.max_internal_rate
        ));
    }
    if !is_supported_internal_sample_rate(args.max_internal_rate) {
        return Err(format!(
            "max-internal must be one of 8000, 12000, 16000, 24000: {}",
            args.max_internal_rate
        ));
    }

    if args.packet_ms % FRAME_LENGTH_MS != 0 {
        return Err(format!(
            "packet-ms must be multiple of {FRAME_LENGTH_MS}: {}",
            args.packet_ms
        ));
    }
    if args.packet_ms < FRAME_LENGTH_MS
        || args.packet_ms > FRAME_LENGTH_MS * MAX_INPUT_FRAMES as i32
    {
        return Err(format!(
            "packet-ms out of range ({}-{}): {}",
            FRAME_LENGTH_MS,
            FRAME_LENGTH_MS * MAX_INPUT_FRAMES as i32,
            args.packet_ms
        ));
    }

    let mut output = open_output_writer(&args.output)?;
    let mut total_encoded_bytes: u64 = 0;
    let mut total_input_samples: u64 = 0;

    if args.tencent {
        output
            .write_all(&[TENCENT_PREFIX])
            .map_err(|e| format!("write output: {e}"))?;
        total_encoded_bytes += 1;
    }
    output
        .write_all(SILK_HEADER.as_bytes())
        .map_err(|e| format!("write output: {e}"))?;
    total_encoded_bytes += SILK_HEADER.len() as u64;

    let mut enc_size: c_int = 0;
    unsafe {
        if SKP_Silk_SDK_Get_Encoder_Size(&mut enc_size) != 0 || enc_size <= 0 {
            return Err("failed to get encoder size".to_string());
        }
    }

    let mut enc_state = vec![0u8; enc_size as usize];
    let mut enc_status = SKP_SILK_SDK_EncControlStruct::default();
    unsafe {
        if SKP_Silk_SDK_InitEncoder(enc_state.as_mut_ptr() as *mut c_void, &mut enc_status) != 0 {
            return Err("failed to init encoder".to_string());
        }
    }

    let enc_control = SKP_SILK_SDK_EncControlStruct {
        api_sample_rate: args.sample_rate,
        max_internal_sample_rate: args.max_internal_rate,
        packet_size: (args.packet_ms * args.sample_rate) / 1000,
        bit_rate: args.bitrate,
        packet_loss_percentage: args.loss,
        complexity: args.complexity,
        use_inband_fec: args.fec,
        use_dtx: args.dtx,
    };

    let frame_samples = (FRAME_LENGTH_MS * args.sample_rate) / 1000;
    let mut smpls_since_last_packet = 0;
    let mut payload = vec![0u8; MAX_BYTES_PER_FRAME * MAX_INPUT_FRAMES];
    let mut buffer = vec![0u8; (frame_samples as usize) * 2];
    let mut samples = vec![0i16; frame_samples as usize];
    let mut packets = 0;

    loop {
        let mut read = 0;
        while read < buffer.len() {
            let n = input.read(&mut buffer[read..]).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            read += n;
        }
        if read == 0 {
            break;
        }
        if read < buffer.len() {
            buffer[read..].fill(0);
        }
        total_input_samples += (read / 2) as u64;

        for (idx, chunk) in buffer.chunks_exact(2).enumerate() {
            samples[idx] = i16::from_le_bytes([chunk[0], chunk[1]]);
        }

        let mut n_bytes: c_short = (MAX_BYTES_PER_FRAME * MAX_INPUT_FRAMES) as c_short;
        let ret = unsafe {
            SKP_Silk_SDK_Encode(
                enc_state.as_mut_ptr() as *mut c_void,
                &enc_control,
                samples.as_ptr(),
                samples.len() as c_int,
                payload.as_mut_ptr(),
                &mut n_bytes,
            )
        };
        if ret != 0 {
            return Err(format!("encode failed: {ret}"));
        }

        smpls_since_last_packet += frame_samples;
        let packet_size_ms = (1000 * enc_control.packet_size) / enc_control.api_sample_rate;
        if (1000 * smpls_since_last_packet) / args.sample_rate == packet_size_ms {
            let size_le = n_bytes.to_le_bytes();
            output
                .write_all(&size_le)
                .map_err(|e| format!("write output: {e}"))?;
            total_encoded_bytes += size_le.len() as u64;
            if n_bytes > 0 {
                output
                    .write_all(&payload[..n_bytes as usize])
                    .map_err(|e| format!("write output: {e}"))?;
                total_encoded_bytes += n_bytes as u64;
            }
            smpls_since_last_packet = 0;
            packets += 1;
            if !args.quiet {
                eprint!("\rPackets encoded: {packets}");
            }
        }
    }

    if !args.tencent {
        let n_bytes: c_short = -1;
        output
            .write_all(&n_bytes.to_le_bytes())
            .map_err(|e| format!("write output: {e}"))?;
        total_encoded_bytes += n_bytes.to_le_bytes().len() as u64;
    }

    if args.stats {
        let duration = total_input_samples as f64 / args.sample_rate as f64;
        let bitrate = if duration > 0.0 {
            (total_encoded_bytes as f64 * 8.0) / duration
        } else {
            0.0
        };
        eprintln!(
            "stats: packets={packets} samples={total_input_samples} duration={duration:.3}s bytes={total_encoded_bytes} bitrate={bitrate:.1}bps"
        );
    }

    if !args.quiet {
        eprintln!();
    }

    Ok(())
}

fn decode(args: DecodeArgs) -> Result<(), String> {
    if args.sample_rate <= 0 || args.sample_rate > MAX_API_FS_HZ {
        return Err(format!(
            "sample-rate out of range (8000-48000): {}",
            args.sample_rate
        ));
    }
    if !is_supported_sample_rate(args.sample_rate) {
        return Err(format!(
            "sample-rate must be one of 8000, 12000, 16000, 24000, 48000: {}",
            args.sample_rate
        ));
    }

    let input_source = open_input(&args.input)?;
    if input_source.wav.is_some() {
        return Err("input appears to be WAV; expected silk".into());
    }
    let mut input = input_source.reader;

    let reference_samples = if let Some(path) = &args.reference {
        Some(load_reference_samples(path, args.sample_rate)?)
    } else {
        None
    };
    let mut output = OutputSink::new(
        &args.output,
        args.sample_rate,
        args.wav,
        args.stats || args.reference.is_some(),
        reference_samples,
    )?;

    let mut first = [0u8; 1];
    input
        .read_exact(&mut first)
        .map_err(|_| "invalid silk header")?;
    if first[0] == TENCENT_PREFIX {
        let mut header = [0u8; SILK_HEADER.len()];
        input
            .read_exact(&mut header)
            .map_err(|_| "invalid silk header")?;
        if header != SILK_HEADER.as_bytes() {
            return Err("invalid silk header".into());
        }
    } else {
        let mut header = [0u8; SILK_HEADER.len() - 1];
        input
            .read_exact(&mut header)
            .map_err(|_| "invalid silk header")?;
        if header != SILK_HEADER.as_bytes()[1..] {
            return Err("invalid silk header".into());
        }
    }

    let mut dec_size: c_int = 0;
    unsafe {
        if SKP_Silk_SDK_Get_Decoder_Size(&mut dec_size) != 0 || dec_size <= 0 {
            return Err("failed to get decoder size".to_string());
        }
    }
    let mut dec_state = vec![0u8; dec_size as usize];
    unsafe {
        if SKP_Silk_SDK_InitDecoder(dec_state.as_mut_ptr() as *mut c_void) != 0 {
            return Err("failed to init decoder".to_string());
        }
    }

    let mut dec_control = SKP_SILK_SDK_DecControlStruct {
        api_sample_rate: args.sample_rate,
        frames_per_packet: 1,
        ..Default::default()
    };

    let max_frame_samples = (MAX_FRAME_LENGTH_MS * MAX_API_FS_HZ / 1000) as usize;
    let mut out = vec![0i16; max_frame_samples];
    let mut payload = vec![0u8; MAX_BYTES_PER_FRAME * MAX_INPUT_FRAMES];
    let frame_samples = (FRAME_LENGTH_MS * args.sample_rate) / 1000;
    let mut packets = 0;

    loop {
        let mut size_buf = [0u8; 2];
        let read_size = input.read(&mut size_buf).map_err(|e| e.to_string())?;
        if read_size == 0 {
            break;
        }
        if read_size < 2 {
            return Err("truncated packet size".into());
        }
        let n_bytes = i16::from_le_bytes(size_buf);
        if n_bytes < 0 {
            break;
        }
        let n_bytes_usize = n_bytes as usize;
        if let Some(stats) = output.stats.as_mut() {
            stats.packets += 1;
            stats.bytes += (n_bytes_usize + 2) as u64;
        }
        if n_bytes_usize > payload.len() {
            payload.resize(n_bytes_usize, 0);
        }
        if n_bytes_usize > 0 {
            input
                .read_exact(&mut payload[..n_bytes_usize])
                .map_err(|_| "truncated payload")?;
        }

        if n_bytes_usize == 0 {
            let frames = if dec_control.frames_per_packet > 0 {
                dec_control.frames_per_packet
            } else {
                1
            };
            for frame_idx in 0..frames {
                let mut out_len: c_short = 0;
                let ret = unsafe {
                    SKP_Silk_SDK_Decode(
                        dec_state.as_mut_ptr() as *mut c_void,
                        &mut dec_control,
                        1,
                        payload.as_ptr(),
                        0,
                        out.as_mut_ptr(),
                        &mut out_len,
                    )
                };
                if ret != 0 {
                    match args.tolerant {
                        TolerantMode::Skip => continue,
                        TolerantMode::Silence => {
                            let remaining = (frames - frame_idx) as usize;
                            let samples = remaining * frame_samples as usize;
                            output.write_silence(samples)?;
                            output.record_frames(remaining as u64);
                            break;
                        }
                        TolerantMode::Off => return Err(format!("decode failed: {ret}")),
                    }
                }
                let out_len = out_len as usize;
                if out_len > out.len() {
                    match args.tolerant {
                        TolerantMode::Skip => continue,
                        TolerantMode::Silence => {
                            let remaining = (frames - frame_idx) as usize;
                            let samples = remaining * frame_samples as usize;
                            output.write_silence(samples)?;
                            output.record_frames(remaining as u64);
                            break;
                        }
                        TolerantMode::Off => return Err("decode frame too large".into()),
                    }
                }
                output.write_samples(&out, out_len)?;
                output.record_frames(1);
            }
        } else {
            let mut frames = 0;
            loop {
                let mut out_len: c_short = 0;
                let ret = unsafe {
                    SKP_Silk_SDK_Decode(
                        dec_state.as_mut_ptr() as *mut c_void,
                        &mut dec_control,
                        0,
                        payload.as_ptr(),
                        n_bytes_usize as c_int,
                        out.as_mut_ptr(),
                        &mut out_len,
                    )
                };
                if ret != 0 {
                    match args.tolerant {
                        TolerantMode::Skip => break,
                        TolerantMode::Silence => {
                            output.write_silence(frame_samples as usize)?;
                            output.record_frames(1);
                            break;
                        }
                        TolerantMode::Off => return Err(format!("decode failed: {ret}")),
                    }
                }
                let out_len = out_len as usize;
                if out_len > out.len() {
                    match args.tolerant {
                        TolerantMode::Skip => break,
                        TolerantMode::Silence => {
                            output.write_silence(frame_samples as usize)?;
                            output.record_frames(1);
                            break;
                        }
                        TolerantMode::Off => return Err("decode frame too large".into()),
                    }
                }
                output.write_samples(&out, out_len)?;
                output.record_frames(1);
                frames += 1;
                if frames > MAX_INPUT_FRAMES {
                    match args.tolerant {
                        TolerantMode::Skip => break,
                        TolerantMode::Silence => {
                            output.write_silence(frame_samples as usize)?;
                            output.record_frames(1);
                            break;
                        }
                        TolerantMode::Off => return Err("too many frames in packet".into()),
                    }
                }
                if dec_control.more_internal_decoder_frames == 0 {
                    break;
                }
            }
        }

        packets += 1;
        if !args.quiet {
            eprint!("\rPackets decoded: {packets}");
        }
    }

    output.finalize()?;

    if output.stats.is_some() {
        output.print_stats(args.sample_rate);
    }

    if !args.quiet {
        eprintln!();
    }

    Ok(())
}

struct OutputSink {
    target: OutputTarget,
    wav: Option<WavState>,
    stats: Option<DecodeStats>,
}

enum OutputTarget {
    File(File),
    Stdout(std::io::Stdout),
}

struct DecodeStats {
    packets: u64,
    frames: u64,
    samples: u64,
    bytes: u64,
    energy: f64,
    ref_energy: f64,
    noise_energy: f64,
    ref_pos: usize,
    reference: Option<Vec<i16>>,
}

struct WavState {
    data_len: u32,
    sample_rate: u32,
}

struct InputSource {
    reader: Box<dyn Read>,
    wav: Option<WavInfo>,
}

struct WavInfo {
    sample_rate: i32,
    channels: u16,
    bits_per_sample: u16,
}

impl OutputTarget {
    fn write_all(&mut self, buf: &[u8]) -> Result<(), String> {
        match self {
            OutputTarget::File(file) => file
                .write_all(buf)
                .map_err(|e| format!("write output: {e}")),
            OutputTarget::Stdout(out) => {
                out.write_all(buf).map_err(|e| format!("write output: {e}"))
            }
        }
    }

    fn seek_start(&mut self) -> Result<(), String> {
        match self {
            OutputTarget::File(file) => file
                .seek(SeekFrom::Start(0))
                .map(|_| ())
                .map_err(|e| format!("seek output: {e}")),
            OutputTarget::Stdout(_) => Err("cannot seek stdout".into()),
        }
    }
}

impl OutputSink {
    fn new(
        path: &PathBuf,
        sample_rate: i32,
        force_wav: bool,
        with_stats: bool,
        reference: Option<Vec<i16>>,
    ) -> Result<Self, String> {
        let is_stdout = path.to_string_lossy() == "-";
        let is_wav = force_wav || has_wav_extension(path.as_path());
        if is_stdout && is_wav {
            return Err("cannot write WAV to stdout".into());
        }
        let mut target = if is_stdout {
            OutputTarget::Stdout(std::io::stdout())
        } else {
            OutputTarget::File(File::create(path).map_err(|e| format!("open output: {e}"))?)
        };
        let wav = if is_wav {
            let state = WavState {
                data_len: 0,
                sample_rate: sample_rate as u32,
            };
            write_wav_header(&mut target, &state)?;
            Some(state)
        } else {
            None
        };
        let stats = if with_stats {
            Some(DecodeStats {
                packets: 0,
                frames: 0,
                samples: 0,
                bytes: 0,
                energy: 0.0,
                ref_energy: 0.0,
                noise_energy: 0.0,
                ref_pos: 0,
                reference,
            })
        } else {
            None
        };
        Ok(Self { target, wav, stats })
    }

    fn write_samples(&mut self, buffer: &[i16], len: usize) -> Result<(), String> {
        let mut bytes = Vec::with_capacity(len * 2);
        if let Some(stats) = &mut self.stats {
            for sample in buffer.iter().take(len) {
                bytes.extend_from_slice(&sample.to_le_bytes());
                let s = *sample as f64;
                stats.energy += s * s;
                stats.samples += 1;
                if let Some(reference) = &stats.reference
                    && stats.ref_pos < reference.len()
                {
                    let r = reference[stats.ref_pos] as f64;
                    stats.ref_energy += r * r;
                    let diff = r - s;
                    stats.noise_energy += diff * diff;
                    stats.ref_pos += 1;
                }
            }
        } else {
            for sample in buffer.iter().take(len) {
                bytes.extend_from_slice(&sample.to_le_bytes());
            }
        }
        self.target.write_all(&bytes)?;
        if let Some(wav) = &mut self.wav {
            wav.data_len = wav.data_len.saturating_add((len * 2) as u32);
        }
        Ok(())
    }

    fn write_silence(&mut self, len_samples: usize) -> Result<(), String> {
        if len_samples == 0 {
            return Ok(());
        }
        let bytes = vec![0u8; len_samples * 2];
        self.target.write_all(&bytes)?;
        if let Some(wav) = &mut self.wav {
            wav.data_len = wav.data_len.saturating_add((len_samples * 2) as u32);
        }
        if let Some(stats) = &mut self.stats {
            stats.samples += len_samples as u64;
            if let Some(reference) = &stats.reference {
                for _ in 0..len_samples {
                    if stats.ref_pos < reference.len() {
                        let r = reference[stats.ref_pos] as f64;
                        stats.ref_energy += r * r;
                        stats.noise_energy += r * r;
                        stats.ref_pos += 1;
                    }
                }
            }
        }
        Ok(())
    }

    fn record_frames(&mut self, frames: u64) {
        if let Some(stats) = &mut self.stats {
            stats.frames += frames;
        }
    }

    fn finalize(&mut self) -> Result<(), String> {
        if let Some(wav) = &self.wav {
            self.target.seek_start()?;
            write_wav_header(&mut self.target, wav)?;
        }
        Ok(())
    }

    fn print_stats(&self, sample_rate: i32) {
        let Some(stats) = &self.stats else {
            return;
        };
        let duration = stats.samples as f64 / sample_rate as f64;
        let bitrate = if duration > 0.0 {
            (stats.bytes as f64 * 8.0) / duration
        } else {
            0.0
        };
        let rms = if stats.samples > 0 {
            (stats.energy / stats.samples as f64).sqrt()
        } else {
            0.0
        };
        let avg_frames = if stats.packets > 0 {
            stats.frames as f64 / stats.packets as f64
        } else {
            0.0
        };
        eprintln!(
            "metrics: packets={} frames={} avg_frames={:.2} samples={} duration={:.3}s bytes={} bitrate={:.1}bps rms={:.2}",
            stats.packets,
            stats.frames,
            avg_frames,
            stats.samples,
            duration,
            stats.bytes,
            bitrate,
            rms
        );
        if stats.ref_energy > 0.0 {
            let energy_ratio = stats.energy / stats.ref_energy;
            let snr = if stats.noise_energy > 0.0 {
                10.0 * (stats.ref_energy / stats.noise_energy).log10()
            } else {
                f64::INFINITY
            };
            eprintln!(
                "metrics: ref_samples={} energy_ratio={:.4} snr_db={:.2}",
                stats.ref_pos, energy_ratio, snr
            );
        }
    }
}

fn write_wav_header(out: &mut OutputTarget, wav: &WavState) -> Result<(), String> {
    let byte_rate = wav.sample_rate * 2;
    let block_align = 2u16;
    let bits_per_sample = 16u16;
    let riff_size = 36u32.saturating_add(wav.data_len);
    out.write_all(b"RIFF")?;
    out.write_all(&riff_size.to_le_bytes())?;
    out.write_all(b"WAVE")?;
    out.write_all(b"fmt ")?;
    out.write_all(&16u32.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&wav.sample_rate.to_le_bytes())?;
    out.write_all(&byte_rate.to_le_bytes())?;
    out.write_all(&block_align.to_le_bytes())?;
    out.write_all(&bits_per_sample.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&wav.data_len.to_le_bytes())?;
    Ok(())
}

fn open_input(path: &PathBuf) -> Result<InputSource, String> {
    let is_stdin = path.to_string_lossy() == "-";
    let mut reader: Box<dyn Read> = if is_stdin {
        Box::new(std::io::stdin())
    } else {
        Box::new(File::open(path).map_err(|e| format!("open input: {e}"))?)
    };
    let mut prefix = [0u8; 12];
    let read = reader
        .read(&mut prefix)
        .map_err(|e| format!("read input: {e}"))?;
    if read == 0 {
        return Err("empty input".into());
    }
    if read >= 12 && &prefix[0..4] == b"RIFF" && &prefix[8..12] == b"WAVE" {
        let (info, data_len) = parse_wav_header(&mut *reader)?;
        let reader = Box::new(reader.take(data_len as u64));
        return Ok(InputSource {
            reader,
            wav: Some(info),
        });
    }
    let prefix_vec = prefix[..read].to_vec();
    let chained = std::io::Cursor::new(prefix_vec).chain(reader);
    Ok(InputSource {
        reader: Box::new(chained),
        wav: None,
    })
}

fn parse_wav_header(reader: &mut dyn Read) -> Result<(WavInfo, u32), String> {
    let mut fmt_info: Option<(u16, u16, u32)> = None;
    let data_len = loop {
        let mut chunk_id = [0u8; 4];
        if reader.read_exact(&mut chunk_id).is_err() {
            return Err("wav: missing chunk header".into());
        }
        let mut size_buf = [0u8; 4];
        reader
            .read_exact(&mut size_buf)
            .map_err(|_| "wav: truncated chunk size")?;
        let size = u32::from_le_bytes(size_buf);
        match &chunk_id {
            b"fmt " => {
                if size < 16 {
                    return Err("wav: fmt chunk too small".into());
                }
                let mut buf = vec![0u8; size as usize];
                reader
                    .read_exact(&mut buf)
                    .map_err(|_| "wav: truncated fmt chunk")?;
                let audio_format = u16::from_le_bytes([buf[0], buf[1]]);
                let channels = u16::from_le_bytes([buf[2], buf[3]]);
                let sample_rate = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
                let bits_per_sample = u16::from_le_bytes([buf[14], buf[15]]);
                if audio_format != 1 {
                    return Err("wav: only PCM format is supported".into());
                }
                if channels != 1 {
                    return Err("wav: only mono is supported".into());
                }
                if bits_per_sample != 16 {
                    return Err("wav: only 16-bit PCM supported".into());
                }
                fmt_info = Some((channels, bits_per_sample, sample_rate));
            }
            b"data" => {
                break size;
            }
            _ => {
                skip_bytes(reader, size as usize)?;
            }
        }
        if size % 2 == 1 {
            skip_bytes(reader, 1)?;
        }
    };
    let (channels, bits_per_sample, sample_rate) = fmt_info.ok_or("wav: missing fmt chunk")?;
    Ok((
        WavInfo {
            sample_rate: sample_rate as i32,
            channels,
            bits_per_sample,
        },
        data_len,
    ))
}

fn skip_bytes(reader: &mut dyn Read, mut size: usize) -> Result<(), String> {
    let mut buf = [0u8; 4096];
    while size > 0 {
        let take = size.min(buf.len());
        reader
            .read_exact(&mut buf[..take])
            .map_err(|_| "wav: truncated chunk")?;
        size -= take;
    }
    Ok(())
}

fn open_output_writer(path: &PathBuf) -> Result<Box<dyn Write>, String> {
    let is_stdout = path.to_string_lossy() == "-";
    if is_stdout {
        Ok(Box::new(std::io::stdout()))
    } else {
        Ok(Box::new(
            File::create(path).map_err(|e| format!("open output: {e}"))?,
        ))
    }
}

fn load_reference_samples(path: &PathBuf, sample_rate: i32) -> Result<Vec<i16>, String> {
    let input = open_input(path)?;
    if let Some(wav) = input.wav {
        if wav.channels != 1 || wav.bits_per_sample != 16 {
            return Err("reference wav must be mono 16-bit".into());
        }
        if wav.sample_rate != sample_rate {
            return Err(format!(
                "reference sample-rate mismatch: {} vs {}",
                wav.sample_rate, sample_rate
            ));
        }
    }
    read_all_i16(input.reader)
}

fn read_all_i16(mut reader: Box<dyn Read>) -> Result<Vec<i16>, String> {
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read input: {e}"))?;
    if bytes.len() % 2 != 0 {
        bytes.pop();
    }
    Ok(bytes
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect())
}

fn has_wav_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("wav"))
        .unwrap_or(false)
}

fn default_max_internal_rate(sample_rate: i32) -> i32 {
    sample_rate.min(MAX_INTERNAL_FS_HZ)
}

fn is_supported_sample_rate(rate: i32) -> bool {
    matches!(rate, 8_000 | 12_000 | 16_000 | 24_000 | 48_000)
}

fn is_supported_internal_sample_rate(rate: i32) -> bool {
    matches!(rate, 8_000 | 12_000 | 16_000 | 24_000)
}

fn main() {
    let mut args = env::args().skip(1);
    let cmd = match args.next() {
        Some(cmd) => cmd,
        None => {
            print_usage();
            std::process::exit(1);
        }
    };

    let result = match cmd.as_str() {
        "encode" => parse_encode_args(args).and_then(encode),
        "decode" => parse_decode_args(args).and_then(decode),
        "--version" => {
            unsafe {
                let ptr = SKP_Silk_SDK_get_version();
                if !ptr.is_null() {
                    let cstr = std::ffi::CStr::from_ptr(ptr);
                    println!("silk-sdk {}", cstr.to_string_lossy());
                }
            }
            Ok(())
        }
        "-h" | "--help" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown command: {other}")),
    };

    if let Err(err) = result {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_pcm(path: &PathBuf, sample_rate: i32, seconds: f64) -> Vec<i16> {
        let samples = (sample_rate as f64 * seconds) as usize;
        let mut data = Vec::with_capacity(samples);
        let freq = 440.0f64;
        let amp = 10_000.0f64;
        for i in 0..samples {
            let t = i as f64 / sample_rate as f64;
            let v = (amp * (2.0 * std::f64::consts::PI * freq * t).sin()) as i16;
            data.push(v);
        }
        let mut bytes = Vec::with_capacity(samples * 2);
        for sample in &data {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        fs::write(path, bytes).expect("write pcm");
        data
    }

    fn read_pcm(path: &PathBuf) -> Vec<i16> {
        let bytes = fs::read(path).expect("read pcm");
        bytes
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect()
    }

    fn write_wav(path: &PathBuf, sample_rate: i32, samples: &[i16]) {
        let mut target = OutputTarget::File(File::create(path).expect("create wav"));
        let state = WavState {
            data_len: (samples.len() * 2) as u32,
            sample_rate: sample_rate as u32,
        };
        write_wav_header(&mut target, &state).expect("write wav header");
        for sample in samples {
            target
                .write_all(&sample.to_le_bytes())
                .expect("write wav data");
        }
    }

    fn write_wav_with_trailing_chunk(
        path: &PathBuf,
        sample_rate: i32,
        samples: &[i16],
        chunk_id: [u8; 4],
        payload: &[u8],
    ) {
        let data_len = (samples.len() * 2) as u32;
        let padded_payload_len = payload.len() + (payload.len() % 2);
        let riff_size = 36u32 + data_len + 8u32 + padded_payload_len as u32;
        let mut bytes = Vec::with_capacity(riff_size as usize + 8);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&riff_size.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate as u32).to_le_bytes());
        bytes.extend_from_slice(&((sample_rate as u32) * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes.extend_from_slice(&chunk_id);
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            bytes.push(0);
        }
        fs::write(path, bytes).expect("write wav with trailing chunk");
    }

    fn assert_roundtrip(tencent: bool) {
        let dir = temp_dir("rust-silk");
        let input = dir.join("input.pcm");
        let silk = dir.join("audio.silk");
        let output = dir.join("output.pcm");
        let sample_rate = 24_000;
        let input_samples = write_pcm(&input, sample_rate, 1.0);

        let enc = EncodeArgs {
            input: input.clone(),
            output: silk.clone(),
            sample_rate,
            sample_rate_set: true,
            max_internal_rate: sample_rate,
            max_internal_set: true,
            packet_ms: 20,
            bitrate: 25_000,
            loss: 0,
            fec: 0,
            dtx: 0,
            complexity: 2,
            tencent,
            stats: false,
            quiet: true,
        };
        encode(enc).expect("encode");

        let dec = DecodeArgs {
            input: silk.clone(),
            output: output.clone(),
            sample_rate,
            wav: false,
            tolerant: TolerantMode::Off,
            stats: false,
            reference: None,
            quiet: true,
        };
        decode(dec).expect("decode");

        let output_samples = read_pcm(&output);
        assert_eq!(
            output_samples.len(),
            input_samples.len(),
            "decoded length mismatch"
        );

        let in_energy: f64 = input_samples
            .iter()
            .map(|s| (*s as f64) * (*s as f64))
            .sum();
        let out_energy: f64 = output_samples
            .iter()
            .map(|s| (*s as f64) * (*s as f64))
            .sum();
        assert!(out_energy > 0.0, "decoded audio is silent");
        let ratio = out_energy / in_energy.max(1.0);
        assert!(ratio > 0.01, "decoded energy too low: {ratio}");
    }

    #[test]
    fn roundtrip_default_header() {
        assert_roundtrip(false);
    }

    #[test]
    fn roundtrip_tencent_header() {
        assert_roundtrip(true);
    }

    #[test]
    fn roundtrip_wav_input() {
        let dir = temp_dir("rust-silk-wav");
        let wav = dir.join("input.wav");
        let silk = dir.join("audio.silk");
        let output = dir.join("output.pcm");
        let sample_rate = 16_000;
        let input_samples = write_pcm(&dir.join("input.pcm"), sample_rate, 1.0);
        write_wav(&wav, sample_rate, &input_samples);

        let enc = EncodeArgs {
            input: wav.clone(),
            output: silk.clone(),
            sample_rate: 24_000,
            sample_rate_set: false,
            max_internal_rate: 24_000,
            max_internal_set: false,
            packet_ms: 20,
            bitrate: 25_000,
            loss: 0,
            fec: 0,
            dtx: 0,
            complexity: 2,
            tencent: false,
            stats: false,
            quiet: true,
        };
        encode(enc).expect("encode wav");

        let dec = DecodeArgs {
            input: silk.clone(),
            output: output.clone(),
            sample_rate,
            wav: false,
            tolerant: TolerantMode::Off,
            stats: false,
            reference: None,
            quiet: true,
        };
        decode(dec).expect("decode wav");

        let output_samples = read_pcm(&output);
        assert_eq!(
            output_samples.len(),
            input_samples.len(),
            "decoded length mismatch for wav input"
        );
    }

    #[test]
    fn roundtrip_48k_input_uses_24k_internal_rate() {
        let dir = temp_dir("rust-silk-48k");
        let input = dir.join("input.pcm");
        let silk = dir.join("audio.silk");
        let output = dir.join("output.pcm");
        let sample_rate = 48_000;
        let input_samples = write_pcm(&input, sample_rate, 0.2);

        let enc = parse_encode_args(
            [
                "--input".to_string(),
                input.to_string_lossy().into_owned(),
                "--output".to_string(),
                silk.to_string_lossy().into_owned(),
                "--sample-rate".to_string(),
                sample_rate.to_string(),
                "--quiet".to_string(),
            ]
            .into_iter(),
        )
        .expect("parse 48k encode args");
        assert_eq!(enc.max_internal_rate, 24_000);

        encode(enc).expect("encode 48k");

        let dec = DecodeArgs {
            input: silk.clone(),
            output: output.clone(),
            sample_rate,
            wav: false,
            tolerant: TolerantMode::Off,
            stats: false,
            reference: None,
            quiet: true,
        };
        decode(dec).expect("decode 48k");

        let output_samples = read_pcm(&output);
        assert_eq!(
            output_samples.len(),
            input_samples.len(),
            "decoded length mismatch for 48k input"
        );
    }

    #[test]
    fn wav_reader_stops_at_data_chunk_boundary() {
        let dir = temp_dir("rust-silk-wav-data-boundary");
        let wav = dir.join("input.wav");
        let sample_rate = 24_000;
        let expected_samples = write_pcm(&dir.join("input.pcm"), sample_rate, 0.1);
        write_wav_with_trailing_chunk(&wav, sample_rate, &expected_samples, *b"JUNK", b"ABCD");

        let input = open_input(&wav).expect("open wav");
        assert!(input.wav.is_some(), "expected WAV metadata");

        let actual_samples = read_all_i16(input.reader).expect("read wav samples");
        assert_eq!(
            actual_samples, expected_samples,
            "reader must stop at declared data chunk length"
        );
    }
}
