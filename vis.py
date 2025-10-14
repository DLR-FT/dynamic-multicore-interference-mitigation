#!/usr/bin/env python3

import json
import matplotlib
import matplotlib.pyplot as plt
import plotly.express as px
import numpy as np
import polars as pl
import sys
import tkinter


def parse_lines(lines):
    data = []
    for  line in lines:
        x = json.loads(line)
        data.append(x)

    data = pl.from_dicts(data)
    return data


with open(sys.argv[1]) as f:
    linesA = f.readlines()

with open(sys.argv[2]) as f:
    linesB = f.readlines()

dataA = parse_lines(linesA)
dataB = parse_lines(linesB)

print(dataA)


fig, axs = plt.subplots(2, 2, sharex="col")

foo = dataA.to_numpy()
k = foo[:, 4]
tpf = foo[:, 5]*1000/foo[:, 6]
ma_tpf = foo[:, 7]
axs[0, 0].set_title(sys.argv[1])
axs[0, 0].plot(k, tpf)
axs[0, 0].plot(k, ma_tpf)
axs[0, 1].hist(tpf, bins=1000, density=True, log=True)


foo = dataB.to_numpy()
k = foo[:, 4]
tpf = foo[:, 5]*1000/foo[:, 6]
ma_tpf = foo[:, 7]
axs[1, 0].set_title(sys.argv[2])
axs[1, 0].plot(k, tpf)
axs[1, 0].plot(k, ma_tpf)
axs[1, 1].hist(tpf, bins=1000, density=True, log=True)


plt.show()