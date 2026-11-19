use bytes::Bytes;

pub(crate) struct Planar<'a> {
    pub width: usize,
    pub height: usize,
    pub y: &'a [u8],
    pub y_stride: usize,
    pub u: &'a [u8],
    pub v: &'a [u8],
    pub uv_stride: usize,
}

pub(crate) fn from_i420(planes: &Planar) -> Option<(Bytes, usize)> {
    let Planar {
        width,
        height,
        y,
        y_stride,
        u,
        v,
        uv_stride,
    } = *planes;
    let chroma_width = width.div_ceil(2);
    let chroma_height = height.div_ceil(2);
    let covers = |plane: &[u8], stride: usize, rows: usize, row_len: usize| {
        rows == 0 || plane.len() >= (rows - 1) * stride + row_len
    };
    if width == 0
        || height == 0
        || !covers(y, y_stride, height, width)
        || !covers(u, uv_stride, chroma_height, chroma_width)
        || !covers(v, uv_stride, chroma_height, chroma_width)
    {
        return None;
    }
    let stride = chroma_width * 2;
    let mut out = vec![0; stride * (height + chroma_height)];
    for (row, line) in out.chunks_exact_mut(stride).take(height).enumerate() {
        line[..width].copy_from_slice(&y[row * y_stride..][..width]);
    }
    for (row, line) in out.chunks_exact_mut(stride).skip(height).enumerate() {
        let u = &u[row * uv_stride..][..chroma_width];
        let v = &v[row * uv_stride..][..chroma_width];
        for (pair, (u, v)) in line.chunks_exact_mut(2).zip(u.iter().zip(v)) {
            pair[0] = *u;
            pair[1] = *v;
        }
    }
    Some((Bytes::from(out), stride))
}

pub(crate) fn narrow_le_bytes(
    plane: &[u8],
    stride: usize,
    width: usize,
    height: usize,
    depth: u32,
) -> Option<Vec<u8>> {
    let shift = depth.saturating_sub(8);
    let mut out = Vec::with_capacity(width * height);
    for row in 0..height {
        let line = plane.get(row * stride..)?.get(..width * 2)?;
        out.extend(
            line.chunks_exact(2)
                .map(|pair| (u16::from_le_bytes([pair[0], pair[1]]) >> shift) as u8),
        );
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn little_endian_bytes_are_narrowed_like_samples() {
        let plane = [0xFF, 0x03, 0x00, 0x02, 9, 9, 9, 9];
        assert_eq!(narrow_le_bytes(&plane, 4, 2, 1, 10).unwrap(), [255, 128]);
    }

    #[test]
    fn a_byte_plane_shorter_than_its_rows_is_rejected() {
        assert!(narrow_le_bytes(&[1, 2, 3], 2, 2, 1, 10).is_none());
    }

    #[test]
    fn chroma_planes_are_interleaved_after_the_luma_rows() {
        let y = [1, 2, 3, 4, 5, 6, 7, 8];
        let (data, stride) = from_i420(&Planar {
            width: 4,
            height: 2,
            y: &y,
            y_stride: 4,
            u: &[10, 11],
            v: &[20, 21],
            uv_stride: 2,
        })
        .unwrap();
        assert_eq!(stride, 4);
        assert_eq!(&data[..], &[1, 2, 3, 4, 5, 6, 7, 8, 10, 20, 11, 21]);
    }

    #[test]
    fn row_padding_in_the_source_is_dropped() {
        let y = [1, 2, 9, 9, 3, 4, 9, 9];
        let (data, _) = from_i420(&Planar {
            width: 2,
            height: 2,
            y: &y,
            y_stride: 4,
            u: &[10, 9, 9],
            v: &[20, 9, 9],
            uv_stride: 3,
        })
        .unwrap();
        assert_eq!(&data[..], &[1, 2, 3, 4, 10, 20]);
    }

    #[test]
    fn odd_sizes_round_the_chroma_up() {
        let (data, stride) = from_i420(&Planar {
            width: 3,
            height: 3,
            y: &[1; 9],
            y_stride: 3,
            u: &[10, 11, 12, 13],
            v: &[20, 21, 22, 23],
            uv_stride: 2,
        })
        .unwrap();
        assert_eq!(stride, 4);
        assert_eq!(data.len(), 4 * 3 + 4 * 2);
        assert_eq!(&data[12..], &[10, 20, 11, 21, 12, 22, 13, 23]);
    }

    #[test]
    fn planes_shorter_than_the_picture_are_rejected() {
        assert!(
            from_i420(&Planar {
                width: 4,
                height: 2,
                y: &[0; 7],
                y_stride: 4,
                u: &[0; 2],
                v: &[0; 2],
                uv_stride: 2,
            })
            .is_none()
        );
    }
}
