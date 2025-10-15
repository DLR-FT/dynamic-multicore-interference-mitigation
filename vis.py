#!/usr/bin/env python3

import json
import matplotlib
import matplotlib.pyplot as plt
import plotly.express as px
import numpy as np
import polars as pl
import polars.selectors as cs
import sys
import tkinter
import altair as alt

alt.renderers.enable("browser")
alt.data_transformers.enable("vegafusion")

def parse_lines(lines):
    data = []
    for  line in lines:
        x = json.loads(line)
        data.append(x)

    data = pl.from_dicts(data)
    return data

def analyze_tpf(data):
    data = data.with_columns(
        tpf=(pl.col("dt") / pl.col("df"))
    ).select(["fuel", "i", "j", "k", "tpf"])
    
    res = []
    for i, data1 in data.group_by("i", maintain_order=True):
        pivot = data1.pivot("j", index="k")
        k = pivot.select(pl.col("k"))
        avg_tpf = pivot.select(cs.starts_with("tpf_")).mean_horizontal()
        res.append(pl.DataFrame({"k": k, "avg_tpf": avg_tpf}))

    return res


with open(sys.argv[1]) as f:
    linesA = f.readlines()

with open(sys.argv[2]) as f:
    linesB = f.readlines()

dataA = parse_lines(linesA)
dataB = parse_lines(linesB)

tpfA = analyze_tpf(dataA)
tpfB = analyze_tpf(dataB)

chart0 = alt.Chart(tpfA[0]).mark_point().encode(x="k",y="avg_tpf").interactive().properties(width="container", height=750)
chart1 = alt.Chart(tpfA[1]).mark_point().encode(x="k",y="avg_tpf").interactive().properties(width="container", height=750)

( chart1).show()