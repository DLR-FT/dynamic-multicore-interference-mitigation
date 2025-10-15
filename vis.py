#!/usr/bin/env python3

import json
import matplotlib
import matplotlib.pyplot as plt
import numpy as np
import polars as pl
import polars.selectors as cs
import sys
import tkinter


def parse_lines(lines):
    data = []
    for  line in lines:
        x = json.loads(line)
        data.append(x)

    data = pl.from_dicts(data)
    return data

def avg_e2e_tpf(data):
    res = []
    for i, data1 in data.group_by("i", maintain_order=True):
        x = data1.group_by("j", maintain_order=True).tail(1).select("j", "avg_tpf")
        res.append(x)

    return res

def tpf(data):
    res = []
    for i, data1 in data.group_by("i", maintain_order=True):
        data1 = data1.with_columns(
            tpf=pl.col("dt")*1000/pl.col("df")
        ).select("j", "k", "tpf").filter(
            pl.col("tpf") < 50000
        )
        res.append(data1)

    return res


with open(sys.argv[1]) as f:
    linesA = f.readlines()

with open(sys.argv[2]) as f:
    linesB = f.readlines()

dataA = parse_lines(linesA)
dataB = parse_lines(linesB)

avg_tpfA = avg_e2e_tpf(dataA)
avg_tpfB = avg_e2e_tpf(dataB)

tpfA = tpf(dataA)
tpfB = tpf(dataB)

print(tpfA)

fig, axs = plt.subplots(2, 2, sharex="col", sharey="col")

for x in avg_tpfA:
    axs[0, 0].hist(x.select("avg_tpf").to_numpy(), bins=100)

for x in avg_tpfB:
    axs[1, 0].hist(x.select("avg_tpf").to_numpy(), bins=100)

for x in tpfA:
    axs[0, 1].hist(x.select("tpf").to_numpy(), bins=1000)

for x in tpfB:
    axs[1, 1].hist(x.select("tpf").to_numpy(), bins=1000)

plt.show()