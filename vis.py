#!/usr/bin/env python3

import sys
import tkinter
import matplotlib
import matplotlib.pyplot as plt
import plotly.express as px
import numpy as np
import polars as pl


with open(sys.argv[1]) as f:
    lines = f.readlines()

data = []
for line in lines:
    point = [int(x) for x in line.strip().split(',')]
    data.append(point)

data = np.array(data)

plt.plot(data[:, 0], data[:, 2])
plt.show()