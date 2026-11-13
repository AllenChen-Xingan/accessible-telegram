# exp007 一次性对照脚本：两份 16 位单声道 WAV，在 ±400 样本内找最佳对齐，算抽样波形信噪比（dB）。
# 抽样口径：跳过开头 1000 个、末尾约 500 个样本，中间每隔 7 个取一个（README「数据」一节）。
# 用法：python3 -I snr.py <A.wav> <B.wav>   （A 当基准；输出帧数、最佳偏移、信噪比）
import wave, sys, array, math

def rd(p):
    w = wave.open(p)
    return array.array('h', w.readframes(w.getnframes()))

a, b = rd(sys.argv[1]), rd(sys.argv[2])
best = None
for lag in range(-400, 401):
    n = min(len(a), len(b)) - 500
    s = e = 0
    for i in range(1000, n, 7):
        j = i + lag
        if 0 <= j < len(b):
            d = a[i] - b[j]
            s += a[i] * a[i]
            e += d * d
    snr = 10 * math.log10(s / max(e, 1))
    if best is None or snr > best[1]:
        best = (lag, snr)
print("frames", len(a), len(b), "best lag", best[0], "SNR dB %.1f" % best[1])
